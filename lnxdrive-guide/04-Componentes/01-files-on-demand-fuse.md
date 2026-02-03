# Files-on-Demand para Linux

> **Ubicación:** `04-Componentes/01-files-on-demand-fuse.md`
> **Relacionado:** [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md)

---

## Parte III: Files-on-Demand para Linux

### 3.1 El Desafio

Windows tiene [Cloud Files API (cfapi)](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine) integrada en el kernel con `cldflt.sys`. Esta API **no existe en Linux** porque depende de caracteristicas especificas de NTFS.

> "Cldflt.sys currently only supports NTFS volumes because it depends on some features unique to NTFS." — Microsoft Documentation

### 3.2 Nuestra Solucion: FUSE + Overlay + GIO

Implementaremos Files-on-Demand usando una combinacion de tecnologias:

```
┌───────────────────────────────────────────────────────────────────┐
│                    CAPA DE PRESENTACION                           │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │  Administrador de Archivos (Nautilus/Dolphin/Thunar)       │   │
│  │  ────────────────────────────────────────────────────────  │   │
│  │  • Overlay icons via GIO/KIO extension                     │   │
│  │  • Menu contextual: "Make available offline"               │   │
│  │  • Indicador de estado: ☁️ online | ✓ local | ⟳ syncing   │   │
│  └────────────────────────────────────────────────────────────┘   │
│                              │                                    │
│                              │ GIO/KIO API                        │
│                              ▼                                    │
├───────────────────────────────────────────────────────────────────┤
│                    CAPA FUSE (Userspace)                          │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  lnxdrive-fuse daemon                                       │    │
│  │  ───────────────────────────────────────────────────────  │    │
│  │  Implementa operaciones FUSE:                             │    │
│  │  • getattr() → Retorna metadata sin descargar contenido   │    │
│  │  • open() → Trigger de hidratacion si es placeholder      │    │
│  │  • read() → Streaming desde cache o desde nube            │    │
│  │  • readdir() → Lista desde cache de metadata              │    │
│  │  • setxattr() → "user.lnxdrive.state" para marcar estado    │    │
│  └───────────────────────────────────────────────────────────┘    │
│                              │                                    │
│                              │ Callbacks al Core                  │
│                              ▼                                    │
├───────────────────────────────────────────────────────────────────┤
│                    NUCLEO DE DOMINIO                              │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  HydrationManager                                           │  │
│  │  ─────────────────────────────────────────────────────────  │  │
│  │  • Gestiona cola de hidratacion con prioridades             │  │
│  │  • Streaming parcial (range requests) para archivos grandes │  │
│  │  • Cache LRU para archivos hidratados recientemente         │  │
│  │  • Dehydration automatica cuando espacio es bajo            │  │
│  └─────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────┘
```

### 3.3 Estados de Archivo

```
┌─────────────────────────────────────────────────────────────────┐
│  PLACEHOLDER (Online-only)                                      │
│  ────────────────────────────────────────────────────────────── │
│  • Archivo sparse de 0 bytes en disco                           │
│  • Metadata completa en extended attributes                     │
│  • xattr: user.lnxdrive.state = "online"                          │
│  • xattr: user.lnxdrive.size = "1234567" (tamano real)            │
│  • xattr: user.lnxdrive.remote_id = "abc123"                      │
│  • Icono: ☁️ nube                                               │
├─────────────────────────────────────────────────────────────────┤
│  HYDRATING (Descargando)                                        │
│  ────────────────────────────────────────────────────────────── │
│  • Archivo parcialmente descargado                              │
│  • xattr: user.lnxdrive.state = "hydrating"                       │
│  • xattr: user.lnxdrive.progress = "45"                           │
│  • Icono: ⟳ sync spinner                                        │
├─────────────────────────────────────────────────────────────────┤
│  HYDRATED (Disponible offline)                                  │
│  ────────────────────────────────────────────────────────────── │
│  • Contenido completo en disco                                  │
│  • xattr: user.lnxdrive.state = "hydrated"                        │
│  • Puede ser dehidratado si espacio es necesario                │
│  • Icono: ✓ check verde                                         │
├─────────────────────────────────────────────────────────────────┤
│  PINNED (Siempre offline)                                       │
│  ────────────────────────────────────────────────────────────── │
│  • Usuario marco explicitamente "Keep on device"                │
│  • xattr: user.lnxdrive.state = "pinned"                          │
│  • Nunca se dehidrata automaticamente                           │
│  • Icono: 📌 pin                                                │
└─────────────────────────────────────────────────────────────────┘
```

### 3.4 Implementacion FUSE Moderna

Para la implementacion FUSE usamos el crate [fuser](https://crates.io/crates/fuser), el binding FUSE mas maduro y activo en el ecosistema Rust.

**Estado Actual de FUSE en Rust:**
- [fuser](https://github.com/cberner/fuser) — Fork activo de rust-fuse, bien mantenido
- Soporte completo para libfuse3
- API sincrona con integracion tokio disponible

**Implementacion: `lnxdrive-fuse`**

```rust
use fuser::{Filesystem, Request, ReplyAttr, ReplyData, ReplyDirectory};
use std::ffi::OsStr;
use std::time::Duration;

/// Filesystem virtual para Files-on-Demand
pub struct LnxDriveFs {
    state_repo: Arc<dyn IStateRepository>,
    hydration_manager: Arc<HydrationManager>,
}

impl Filesystem for LnxDriveFs {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        // Retorna metadata sin descargar contenido
        match self.state_repo.get_item_by_inode(ino) {
            Some(item) => reply.attr(&Duration::from_secs(1), &item.to_file_attr()),
            None => reply.error(libc::ENOENT),
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64,
            offset: i64, size: u32, _flags: i32, _lock: Option<u64>, reply: ReplyData) {
        // Hidrata on-demand si es necesario
        let data = self.hydration_manager.read_with_hydration(ino, offset, size);
        match data {
            Ok(bytes) => reply.data(&bytes),
            Err(e) => reply.error(e.to_errno()),
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64,
               offset: i64, mut reply: ReplyDirectory) {
        // Lista desde cache de metadata (sin descargas)
        for (i, entry) in self.state_repo.list_children(ino).skip(offset as usize).enumerate() {
            if reply.add(entry.ino, (offset + i as i64 + 1), entry.kind, &entry.name) {
                break;
            }
        }
        reply.ok();
    }
}

// Caracteristicas:
// • Sin GC: latencia predecible <1ms para getattr
// • Zero-copy con slices para operaciones de lectura
// • Integracion con tokio para I/O asincrono
// • Extended attributes via xattr para estado de sync
```

---

## ⚠️ Riesgos y Mitigaciones

Esta sección documenta riesgos identificados durante la simulación arquitectónica y sus mitigaciones propuestas.

### A2: SQLite ↔ FUSE Race Condition

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P0 (Crítica) |
| **Componentes** | FUSE daemon, SQLite, HydrationManager |
| **Simulación** | SIM-L3-001 |

**Descripción:**
SQLite solo soporta un escritor simultáneo. Cuando FUSE procesa operaciones de lectura/escritura concurrentes mientras el sync engine actualiza estado, puede ocurrir `SQLITE_BUSY` o corrupción silenciosa si no se maneja correctamente.

**Escenarios de Fallo:**
1. Usuario abre archivo mientras sync engine actualiza metadata
2. Hidratación concurrente de múltiples archivos
3. Checkpoint WAL durante operación FUSE

**Mitigación Propuesta:**
```rust
// Write serialization layer
pub struct WriteSerializer {
    tx: mpsc::Sender<WriteOp>,
}

impl WriteSerializer {
    pub async fn execute(&self, op: WriteOp) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((op, tx)).await?;
        rx.await?
    }
}
```

**Tests Requeridos:**
- `test_concurrent_fuse_sqlite_writes`
- `test_wal_checkpoint_during_hydration`

---

### C1: Write During Hydration

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P0 (Crítica) |
| **Componentes** | FUSE `write()`, HydrationManager |
| **Simulación** | SIM-L2-002 |

**Descripción:**
Si un usuario intenta escribir en un archivo mientras está siendo hidratado (descargado), puede ocurrir corrupción de datos o pérdida de la escritura del usuario.

**Escenarios de Fallo:**
1. `open()` dispara hidratación, `write()` llega antes de completar
2. Aplicación hace `read()` + `write()` rápido (ej: editor de texto)
3. Múltiples procesos acceden al mismo archivo

**Mitigación Propuesta:**
```rust
impl Filesystem for LnxDriveFs {
    fn write(&mut self, _req: &Request, ino: u64, ...) {
        // Bloqueo exclusivo durante hidratación
        let guard = self.hydration_manager.acquire_write_lock(ino)?;
        
        if guard.is_hydrating() {
            // Opción 1: Bloquear hasta completar
            guard.wait_for_hydration()?;
            
            // Opción 2: Retornar EAGAIN
            // return reply.error(libc::EAGAIN);
        }
        
        // Proceder con escritura...
    }
}
```

**Tests Requeridos:**
- `test_write_blocked_during_hydration`
- `test_concurrent_open_write_same_file`

---

### C2: Dehydration Race Condition

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P2 (Media) |
| **Componentes** | FUSE, Dehydration policy, file handles |
| **Simulación** | SIM-L2-003 |

**Descripción:**
El sistema puede intentar dehidratar (eliminar contenido local) un archivo mientras otro proceso lo tiene abierto para lectura, causando errores de I/O inesperados.

**Escenarios de Fallo:**
1. Política de espacio bajo dispara dehidratación mientras usuario ve documento
2. Proceso de larga duración (ej: video player) tiene handle abierto
3. Dehidratación durante indexación de búsqueda

**Mitigación Propuesta:**
```rust
pub struct DehydrationPolicy {
    open_handles: Arc<DashMap<u64, AtomicUsize>>,
}

impl DehydrationPolicy {
    pub fn can_dehydrate(&self, ino: u64) -> bool {
        match self.open_handles.get(&ino) {
            Some(count) if count.load(Ordering::SeqCst) > 0 => false,
            _ => true,
        }
    }
    
    pub fn track_open(&self, ino: u64) {
        self.open_handles
            .entry(ino)
            .or_insert(AtomicUsize::new(0))
            .fetch_add(1, Ordering::SeqCst);
    }
}
```

**Tests Requeridos:**
- `test_dehydration_waits_for_readers`
- `test_long_running_process_prevents_dehydration`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)
> - [RISK-001-critical-paths.md](../.devtrail/02-design/risk-analysis/RISK-001-critical-paths.md)
>
> Diagramas de secuencia relacionados:
> - [SEQ-001-fuse-hydration-race.puml](../.devtrail/02-design/diagrams/SEQ-001-fuse-hydration-race.puml)

---

## Ver tambien

- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Visión general del sistema
- [Adaptador GNOME](02-ui-gnome.md) - Integracion con GNOME
- [Adaptador KDE Plasma](03-ui-kde-plasma.md) - Integracion con KDE
- [CLI Universal](06-cli.md) - Interfaz de linea de comandos

