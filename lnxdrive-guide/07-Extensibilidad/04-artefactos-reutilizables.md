# Artefactos Reutilizables

> **Ubicacion:** `07-Extensibilidad/04-artefactos-reutilizables.md`
> **Relacionado:** [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md), [Puerto ICloudProvider](02-puerto-icloudprovider.md)

---

## Parte XIV: Artefactos Reutilizables

### 14.1 Filosofia de Desacoplamiento

La arquitectura hexagonal no solo beneficia a LNXDrive internamente, sino que permite **extraer componentes como librerias independientes** que pueden ser utiles para otros proyectos de sincronizacion, backup o gestion de archivos.

### 14.2 Catalogo de Artefactos Desacoplables

```
┌─────────────────────────────────────────────────────────────────────┐
│  ARTEFACTOS REUTILIZABLES                                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ████ ALTA REUTILIZACION (publicable como crate/paquete) ████       │
│                                                                     │
│  lnxdrive-fuse                                                        │
│  ────────────────────────────────────────────────────────────────── │
│  Wrapper FUSE moderno para Rust con soporte para:                   │
│  • Async/await nativo (tokio)                                       │
│  • Extended attributes                                              │
│  • Placeholders (archivos sparse con metadata)                      │
│  • Zero-copy con slices y referencias                               │
│  Uso: Cualquier filesystem virtual (no solo sync de nube)           │
│                                                                     │
│  lnxdrive-audit                                                       │
│  ────────────────────────────────────────────────────────────────── │
│  Sistema de auditoria estructurada con:                             │
│  • Eventos tipados con contexto                                     │
│  • Storage pluggable (SQLite, archivo, etc.)                        │
│  • Query API para historial                                         │
│  • Explicaciones legibles para usuarios                             │
│  Uso: Cualquier app que necesite audit trail                        │
│                                                                     │
│  lnxdrive-ratelimit                                                   │
│  ────────────────────────────────────────────────────────────────── │
│  Rate limiter adaptativo con:                                       │
│  • Token bucket por endpoint                                        │
│  • Backpressure con colas priorizadas                               │
│  • Auto-ajuste basado en respuestas 429                             │
│  • Metricas Prometheus incluidas                                    │
│  Uso: Cualquier cliente de API con limites de rate                  │
│                                                                     │
│  lnxdrive-conflict                                                    │
│  ────────────────────────────────────────────────────────────────── │
│  Motor de deteccion y resolucion de conflictos:                     │
│  • Deteccion basada en hashes y timestamps                          │
│  • Estrategias configurables (keep-local, keep-remote, merge)       │
│  • Sistema de reglas por patron glob                                │
│  • Hooks para resolucion personalizada                              │
│  Uso: Sync bidireccional, merge de configuraciones                  │
│                                                                     │
│  ████ MEDIA REUTILIZACION (util como referencia/fork) ████          │
│                                                                     │
│  lnxdrive-overlay                                                     │
│  ────────────────────────────────────────────────────────────────── │
│  Extensiones para file managers:                                    │
│  • Nautilus extension (Python)                                      │
│  • Dolphin ServiceMenu                                              │
│  • Overlay icons via GIO/KIO                                        │
│  Uso: Cualquier app que quiera integrar con file managers           │
│                                                                     │
│  lnxdrive-state                                                       │
│  ────────────────────────────────────────────────────────────────── │
│  Maquina de estados para items sincronizables:                      │
│  • Estados: Online → Hydrating → Hydrated → Modified → Synced       │
│  • Transiciones validadas                                           │
│  • Persistencia pluggable                                           │
│  Uso: Cualquier sistema de sync con estados de archivo              │
│                                                                     │
│  ████ ESPECIFICO DEL PROYECTO ████                                  │
│                                                                     │
│  lnxdrive-graph                                                       │
│  ────────────────────────────────────────────────────────────────── │
│  Adaptador Microsoft Graph optimizado para sync                     │
│  (especifico OneDrive, no reutilizable directamente)                │
│                                                                     │
│  lnxdrive-core                                                        │
│  ────────────────────────────────────────────────────────────────── │
│  Logica de negocio especifica de LNXDrive                             │
│  (orquestacion, UI adapters, etc.)                                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 14.3 Diseno para Reutilizacion

Para que un artefacto sea verdaderamente reutilizable, debe cumplir:

```rust
// ✅ BUENO: API generica sin dependencias al dominio
pub trait AuditStore {
    type Event: AuditEvent;
    type Query: AuditQuery;

    fn record(&self, event: Self::Event) -> Result<EventId>;
    fn query(&self, query: Self::Query) -> Result<Vec<Self::Event>>;
}

// ❌ MALO: API acoplada al dominio OneDrive
pub trait AuditStore {
    fn record_sync_event(&self, file: &OneDriveFile, action: SyncAction);
    fn query_onedrive_history(&self, path: &OneDrivePath) -> Vec<SyncEvent>;
}
```

### 14.4 Ejemplo: lnxdrive-conflict como Libreria

```rust
// Cargo.toml de lnxdrive-conflict
[package]
name = "lnxdrive-conflict"
version = "0.1.0"
description = "Conflict detection and resolution engine for file synchronization"
license = "MIT OR Apache-2.0"
categories = ["filesystem", "data-structures"]
keywords = ["sync", "conflict", "merge", "files"]

[dependencies]
glob = "0.3"
sha2 = "0.10"
serde = { version = "1.0", features = ["derive"] }

[dev-dependencies]
tempfile = "3.0"
```

```rust
// src/lib.rs
//! # lnxdrive-conflict
//!
//! Motor de deteccion y resolucion de conflictos para sincronizacion de archivos.
//!
//! ## Ejemplo basico
//!
//! ```rust
//! use lnxdrive_conflict::{ConflictEngine, Strategy, Rule};
//!
//! let engine = ConflictEngine::builder()
//!     .default_strategy(Strategy::Prompt)
//!     .rule(Rule::new("**/*.docx", Strategy::KeepBoth))
//!     .rule(Rule::new("**/config/**", Strategy::KeepRemote))
//!     .build();
//!
//! let conflict = engine.detect(&local_item, &remote_item)?;
//!
//! match conflict {
//!     Some(c) => {
//!         let resolution = engine.resolve(&c, Strategy::KeepLocal)?;
//!         println!("Resolved: {:?}", resolution);
//!     }
//!     None => println!("No conflict detected"),
//! }
//! ```

pub struct ConflictEngine {
    default_strategy: Strategy,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    KeepLocal,
    KeepRemote,
    KeepBoth,
    Prompt,
    Custom(fn(&Conflict) -> Resolution),
}

pub struct Rule {
    pattern: glob::Pattern,
    strategy: Strategy,
}

pub struct Conflict {
    pub path: PathBuf,
    pub local_version: Version,
    pub remote_version: Version,
    pub detected_at: SystemTime,
}

pub struct Version {
    pub hash: [u8; 32],
    pub modified: SystemTime,
    pub size: u64,
}

pub enum Resolution {
    UseLocal,
    UseRemote,
    UseBoth { local_rename: PathBuf },
    Merged { content: Vec<u8> },
}

impl ConflictEngine {
    pub fn detect(&self, local: &Item, remote: &Item) -> Option<Conflict> {
        // Deteccion basada en hash y timestamps
        if local.hash == remote.hash {
            return None; // Sin conflicto
        }

        if local.modified <= local.last_sync && remote.modified <= local.last_sync {
            return None; // Ambos sin cambios desde ultimo sync
        }

        if local.modified > local.last_sync && remote.modified > local.last_sync {
            // Ambos modificados despues del ultimo sync → CONFLICTO
            Some(Conflict {
                path: local.path.clone(),
                local_version: Version::from(local),
                remote_version: Version::from(remote),
                detected_at: SystemTime::now(),
            })
        } else {
            None // Solo uno modificado, no es conflicto
        }
    }

    pub fn resolve(&self, conflict: &Conflict, strategy: Strategy) -> Result<Resolution> {
        // Aplicar estrategia
        match strategy {
            Strategy::KeepLocal => Ok(Resolution::UseLocal),
            Strategy::KeepRemote => Ok(Resolution::UseRemote),
            Strategy::KeepBoth => {
                let rename = self.generate_conflict_name(&conflict.path);
                Ok(Resolution::UseBoth { local_rename: rename })
            }
            Strategy::Prompt => Err(Error::UserInputRequired),
            Strategy::Custom(resolver) => Ok(resolver(conflict)),
        }
    }

    pub fn get_strategy_for_path(&self, path: &Path) -> Strategy {
        for rule in &self.rules {
            if rule.pattern.matches_path(path) {
                return rule.strategy;
            }
        }
        self.default_strategy
    }
}
```

### 14.5 Publicacion y Versionado

```
┌─────────────────────────────────────────────────────────────────────┐
│  ESTRATEGIA DE PUBLICACION                                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Repositorio mono-repo con workspaces:                              │
│                                                                     │
│  lnxdrive/                                                            │
│  ├── Cargo.toml              (workspace root)                       │
│  ├── crates/                                                        │
│  │   ├── lnxdrive-fuse/        → publicado como crate independiente   │
│  │   ├── lnxdrive-audit/       → publicado como crate independiente   │
│  │   ├── lnxdrive-ratelimit/   → publicado como crate independiente   │
│  │   ├── lnxdrive-conflict/    → publicado como crate independiente   │
│  │   ├── lnxdrive-state/       → publicado como crate independiente   │
│  │   ├── lnxdrive-graph/       → interno (no publicado)               │
│  │   └── lnxdrive-core/        → interno (no publicado)               │
│  └── apps/                                                          │
│      ├── lnxdrive-daemon/      → binario principal                    │
│      ├── lnxdrive-cli/         → binario CLI                          │
│      └── lnxdrive-gnome/       → adaptador GNOME                      │
│                                                                     │
│  Versionado:                                                        │
│  • Crates publicos: semver independiente                            │
│  • Crates internos: version del proyecto                            │
│  • Apps: version unificada del release                              │
│                                                                     │
│  Publicacion:                                                       │
│  • crates.io para Rust                                              │
│  • Documentacion en docs.rs                                         │
│  • Ejemplos en cada crate                                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Arquitectura Multi-Proveedor](01-arquitectura-multi-proveedor.md) - Vision general de extensibilidad
- [Puerto ICloudProvider](02-puerto-icloudprovider.md) - Definicion del puerto y adaptadores
- [Multi-Cuenta con Namespaces](03-multi-cuenta-namespaces.md) - Soporte para multiples cuentas
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Fundamentos arquitectonicos
