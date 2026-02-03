# Estrategia de File Watching con inotify

> **Ubicacion:** `04-Componentes/10-file-watching-inotify.md`
> **Relacionado:** [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]], [[03-Arquitectura/01-arquitectura-hexagonal|Arquitectura Hexagonal]]

---

## Parte XVI: Estrategia de File Watching con inotify

> **Objetivo:** Implementar una estrategia robusta de observacion de archivos que gestione inteligentemente los limites de inotify, reserve recursos para otras aplicaciones del sistema, y proporcione fallback gracioso a polling cuando sea necesario.

### 16.1 IFileObserver Port (Arquitectura Hexagonal)

El puerto `IFileObserver` define el contrato para observacion de cambios en el sistema de archivos, siguiendo los principios de arquitectura hexagonal establecidos en las partes anteriores.

```rust
/// Port para observacion de cambios en el sistema de archivos
pub trait IFileObserver: Send + Sync {
    /// Inicia la observacion del sistema de archivos
    fn start(&self) -> Result<(), FileObserverError>;

    /// Detiene la observacion
    fn stop(&self) -> Result<(), FileObserverError>;

    /// Agrega una ruta a observar con prioridad
    fn watch(&self, path: &Path, priority: WatchPriority) -> Result<WatchHandle, FileObserverError>;

    /// Remueve una observacion
    fn unwatch(&self, handle: WatchHandle) -> Result<(), FileObserverError>;

    /// Obtiene estadisticas actuales
    fn stats(&self) -> WatchStats;

    /// Suscripcion a eventos de cambio
    fn subscribe(&self) -> Receiver<FileChangeEvent>;

    /// Actualiza prioridad de un watch existente
    fn update_priority(&self, handle: WatchHandle, priority: WatchPriority) -> Result<(), FileObserverError>;

    /// Fuerza rebalanceo de watches segun prioridades
    fn rebalance(&self) -> Result<RebalanceReport, FileObserverError>;
}

/// Errores posibles en el sistema de file watching
#[derive(Debug, thiserror::Error)]
pub enum FileObserverError {
    #[error("No hay watches disponibles: {active}/{limit}")]
    WatchLimitReached { active: u32, limit: u32 },

    #[error("Path no encontrado: {0}")]
    PathNotFound(PathBuf),

    #[error("Handle invalido: {0:?}")]
    InvalidHandle(WatchHandle),

    #[error("Error de inotify: {0}")]
    InotifyError(#[from] std::io::Error),

    #[error("El observador ya esta ejecutandose")]
    AlreadyRunning,

    #[error("El observador no esta ejecutandose")]
    NotRunning,
}
```

### 16.2 Estructuras de Datos

#### Prioridades de Watch

Las prioridades determinan que paths mantienen watches nativos cuando se alcanza el limite:

```rust
/// Prioridad de observacion para un path
/// Valores mas altos = mayor prioridad = menos probable de ser eviccionado
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum WatchPriority {
    /// Candidatos para polling (seran eviccionados primero)
    PollingCandidate = 0,

    /// Sin acceso reciente (> 24 horas)
    Stale = 20,

    /// Accedidos en las ultimas 24 horas
    ModeratelyAccessed = 40,

    /// Accedidos en la ultima hora
    RecentlyAccessed = 60,

    /// Archivos abiertos actualmente por el usuario
    ActivelyOpen = 80,

    /// Carpetas raiz de sincronizacion
    SyncRoot = 90,

    /// Archivos/carpetas pinneados por el usuario
    Pinned = 100,
}

impl WatchPriority {
    /// Calcula prioridad basada en tiempo desde ultimo acceso
    pub fn from_last_access(elapsed: Duration) -> Self {
        match elapsed.as_secs() {
            0..=3600 => Self::RecentlyAccessed,      // < 1 hora
            3601..=86400 => Self::ModeratelyAccessed, // 1-24 horas
            _ => Self::Stale,                         // > 24 horas
        }
    }
}
```

#### Estadisticas de Watch

```rust
/// Estadisticas del sistema de file watching
#[derive(Debug, Clone, Default)]
pub struct WatchStats {
    /// Watches inotify activos
    pub active_watches: u32,

    /// Limite configurado para LNXDrive
    pub max_watches: u32,

    /// Limite del sistema (/proc/sys/fs/inotify/max_user_watches)
    pub system_limit: u32,

    /// Watches reservados para otras aplicaciones
    pub reserved_for_system: u32,

    /// Memoria estimada en uso (bytes) - ~1KB por watch
    pub memory_usage_bytes: u64,

    /// Paths actualmente en modo polling
    pub polling_paths: u32,

    /// Intervalo de polling actual (ms)
    pub polling_interval_ms: u32,

    /// Eventos procesados en la ultima hora
    pub events_last_hour: u64,

    /// Eviccciones realizadas en la ultima hora
    pub evictions_last_hour: u32,
}

impl WatchStats {
    /// Porcentaje de utilizacion de watches
    pub fn utilization_percent(&self) -> f32 {
        if self.max_watches == 0 {
            return 0.0;
        }
        (self.active_watches as f32 / self.max_watches as f32) * 100.0
    }

    /// Watches disponibles
    pub fn available_watches(&self) -> u32 {
        self.max_watches.saturating_sub(self.active_watches)
    }

    /// Memoria estimada en MB
    pub fn memory_usage_mb(&self) -> f64 {
        self.memory_usage_bytes as f64 / (1024.0 * 1024.0)
    }
}
```

#### Informacion de Watch Individual

```rust
/// Informacion detallada de un watch especifico
#[derive(Debug, Clone)]
pub struct WatchInfo {
    /// Handle unico del watch
    pub handle: WatchHandle,

    /// Path observado
    pub path: PathBuf,

    /// Prioridad actual
    pub priority: WatchPriority,

    /// Modo de observacion
    pub mode: WatchMode,

    /// Ultimo acceso al path
    pub last_access: SystemTime,

    /// Contador de accesos (para LFU)
    pub access_count: u64,

    /// Timestamp del ultimo evento recibido
    pub last_event: Option<SystemTime>,

    /// Si fue pinneado manualmente por el usuario
    pub is_pinned: bool,
}

/// Handle opaco para identificar un watch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchHandle(pub(crate) u64);

/// Modo de observacion para un path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    /// Usando inotify nativo del kernel
    Native,

    /// Usando polling por falta de watches disponibles
    Polling {
        /// Intervalo de polling en milisegundos
        interval_ms: u32,
    },

    /// Hibrido: inotify para carpeta, polling para archivos dentro
    Hybrid {
        /// Intervalo de polling para archivos (ms)
        file_poll_interval_ms: u32,
    },
}
```

### 16.3 WatchManager: Gestion Inteligente de Watches

El `WatchManager` implementa el algoritmo de asignacion y eviccion de watches:

```rust
use std::collections::{BTreeMap, HashMap, HashSet};
use std::cmp::Ordering;
use std::sync::{Arc, RwLock};
use lru::LruCache;

/// Gestor principal de watches con eviccion LRU/LFU
pub struct WatchManager {
    /// Adaptador inotify (inyectado para testabilidad)
    inotify_adapter: Arc<dyn InotifyAdapter>,

    /// Registro de watches activos (ordenado por prioridad)
    watches: RwLock<BTreeMap<WatchHandle, WatchInfo>>,

    /// Indice inverso: path -> handle
    path_index: RwLock<HashMap<PathBuf, WatchHandle>>,

    /// Cola LRU para eviccion
    lru_queue: RwLock<LruCache<WatchHandle, ()>>,

    /// Contadores de frecuencia para LFU
    access_frequency: RwLock<HashMap<WatchHandle, AccessStats>>,

    /// Configuracion
    config: WatchConfig,

    /// Canal de eventos
    event_sender: tokio::sync::broadcast::Sender<FileChangeEvent>,

    /// Paths en modo polling
    polling_paths: RwLock<HashSet<PathBuf>>,

    /// Generador de handles
    next_handle: std::sync::atomic::AtomicU64,
}

/// Estadisticas de acceso para algoritmo LFU
#[derive(Debug, Clone)]
pub struct AccessStats {
    /// Numero total de accesos
    pub access_count: u64,

    /// Timestamp del ultimo acceso
    pub last_access: SystemTime,

    /// Timestamp de creacion del watch
    pub created_at: SystemTime,
}

impl AccessStats {
    /// Calcula score LFU con decaimiento temporal exponencial
    /// Score mas alto = mas activo = menos probable de eviccion
    pub fn score(&self) -> f64 {
        let age_hours = self.last_access
            .elapsed()
            .map(|d| d.as_secs_f64() / 3600.0)
            .unwrap_or(0.0);

        // Decaimiento exponencial: score baja a la mitad cada 6 horas
        let decay = (-0.115 * age_hours).exp();

        self.access_count as f64 * decay
    }
}
```

### 16.4 Configuracion YAML

La configuracion de file watching se integra en el archivo principal de configuracion:

```yaml
# ~/.config/lnxdrive/config.yaml
file_watching:
  # Estrategia principal:
  # - "native": Solo inotify (fallback automatico a polling si se agota)
  # - "polling": Solo polling (no usa inotify)
  # - "hybrid": inotify para carpetas, polling para archivos
  # - "auto": Seleccion automatica basada en numero de archivos (recomendado)
  strategy: auto

  # Configuracion de limites de inotify
  limits:
    # Porcentaje del limite del sistema a reservar para otras aplicaciones
    # Ejemplo: sistema tiene 8192 watches, con 20% reservado = 6553 para LNXDrive
    system_reserve_percent: 20

    # Maximo absoluto de watches (0 = usar calculo automatico)
    # Si se especifica, nunca se excedera este valor
    max_watches: 0

    # Minimo garantizado de watches para LNXDrive
    # Util cuando otras apps consumen muchos watches
    min_watches: 1000

  # Configuracion de polling (fallback cuando inotify no disponible)
  polling:
    # Intervalo base para paths en polling (milisegundos)
    base_interval_ms: 5000

    # Intervalo maximo con backoff adaptativo
    # El intervalo aumenta si no hay cambios detectados
    max_interval_ms: 60000

    # Factor de backoff cuando no hay cambios
    backoff_multiplier: 1.5

    # Numero de ciclos sin cambios antes de aplicar backoff
    backoff_threshold: 5

    # Patrones glob de paths que siempre usan polling
    # Util para directorios con muchos archivos temporales
    always_poll:
      - "**/node_modules/**"
      - "**/.git/objects/**"
      - "**/target/**"
      - "**/build/**"
      - "**/__pycache__/**"
      - "**/venv/**"

  # Configuracion de prioridades
  priorities:
    # Carpetas que siempre tienen maxima prioridad (no se eviccionan)
    pinned_paths:
      - "~/Documents/Important"
      - "~/Projects/active"

    # Patrones a ignorar completamente (ni inotify ni polling)
    ignore_patterns:
      - "*.tmp"
      - "*.swp"
      - "*.swo"
      - ".~lock.*"
      - "~$*"           # Archivos temporales de Office
      - "*.part"        # Descargas parciales
      - ".DS_Store"
      - "Thumbs.db"

    # Tiempo (segundos) antes de degradar prioridad por inactividad
    stale_threshold_seconds: 86400  # 24 horas

  # Configuracion de memoria
  memory:
    # Mostrar warning en logs si memoria de watches supera este valor (MB)
    warning_threshold_mb: 50

    # Limite duro de memoria para watches (MB, 0 = sin limite)
    # Si se alcanza, se eviccionaran watches hasta bajar del limite
    hard_limit_mb: 0

    # Bytes estimados por watch (para calculos)
    # El kernel tipicamente usa 540-1080 bytes
    bytes_per_watch: 1024

  # Configuracion de eventos
  events:
    # Debounce para eventos rapidos del mismo archivo (ms)
    # Evita procesar multiples eventos de una misma escritura
    debounce_ms: 100

    # Ventana de coalescing para eventos del mismo archivo (ms)
    # Combina multiples eventos en uno solo
    coalesce_window_ms: 500

    # Capacidad del canal de eventos interno
    channel_capacity: 10000

    # Eventos a ignorar por tipo
    ignore_events:
      - access  # Solo lectura, no nos interesa
```

### 16.5 Comunicacion FUSE <-> Core

El sistema FUSE notifica al Core sobre accesos a archivos para actualizar prioridades:

```rust
use std::path::PathBuf;

/// Mensajes desde FUSE hacia el Core
#[derive(Debug, Clone)]
pub enum FuseToCoreMessage {
    /// Archivo abierto por una aplicacion
    FileOpened {
        path: PathBuf,
        /// PID del proceso que abrio el archivo
        pid: u32,
        /// Modo de apertura (lectura, escritura, ambos)
        mode: OpenMode,
    },

    /// Archivo cerrado
    FileClosed {
        path: PathBuf,
        pid: u32,
        /// Si hubo modificaciones durante la sesion
        was_modified: bool,
    },

    /// Directorio listado (readdir)
    DirectoryListed {
        path: PathBuf,
        /// Numero de entradas retornadas
        entry_count: u32,
    },

    /// Atributos consultados (getattr)
    AttributesRead {
        path: PathBuf,
    },

    /// Escritura completada localmente
    LocalWrite {
        path: PathBuf,
        /// Tamano escrito
        size: u64,
        /// Si la escritura esta completa o es parcial
        is_complete: bool,
    },

    /// Archivo eliminado localmente
    LocalDelete {
        path: PathBuf,
    },

    /// Archivo/carpeta creado localmente
    LocalCreate {
        path: PathBuf,
        is_directory: bool,
    },

    /// Archivo/carpeta renombrado
    LocalRename {
        old_path: PathBuf,
        new_path: PathBuf,
    },
}

/// Mensajes desde Core hacia FUSE
#[derive(Debug, Clone)]
pub enum CoreToFuseMessage {
    /// Archivo modificado en el servidor remoto
    RemoteChange {
        path: PathBuf,
        change_type: RemoteChangeType,
        /// Nueva metadata si aplica
        new_metadata: Option<FileMetadata>,
    },

    /// Invalidar cache de directorio (forzar re-lectura)
    InvalidateDirectory {
        path: PathBuf,
    },

    /// Forzar re-descarga de archivo
    ForceRefresh {
        path: PathBuf,
        /// Razon del refresh
        reason: RefreshReason,
    },

    /// Archivo eliminado remotamente
    RemoteDelete {
        path: PathBuf,
    },

    /// Conflicto detectado
    ConflictDetected {
        path: PathBuf,
        conflict_type: ConflictType,
        resolution_path: Option<PathBuf>,
    },
}
```

### 16.6 Comandos CLI

Comandos para gestion y diagnostico de file watching:

```bash
# ═══════════════════════════════════════════════════════════════════
# ESTADISTICAS GENERALES
# ═══════════════════════════════════════════════════════════════════

$ lnxdrive watch status

+===================================================================+
|                    FILE WATCHING STATUS                            |
+===================================================================+
|  Strategy: auto (currently using: native)                         |
|                                                                   |
|  Watches:                                                         |
|    Active:     4,523 / 6,553 (69%)                                |
|    System:     8,192                                              |
|    Reserved:   1,639 (20%)                                        |
|                                                                   |
|  Memory:                                                          |
|    Usage:      4.8 MB (estimated)                                 |
|    Warning at: 50 MB                                              |
|                                                                   |
|  Polling:                                                         |
|    Paths:      127                                                |
|    Interval:   5,000 - 30,000 ms                                  |
|                                                                   |
|  Events (last hour):                                              |
|    Received:   1,234                                              |
|    Coalesced:  456                                                |
|    Evictions:  12                                                 |
+===================================================================+

# ═══════════════════════════════════════════════════════════════════
# LISTAR WATCHES
# ═══════════════════════════════════════════════════════════════════

$ lnxdrive watch list --limit 10

HANDLE  PRIORITY         MODE      LAST ACCESS  PATH
─────────────────────────────────────────────────────────────────────
0x001   Pinned          Native    2 min ago    ~/Documents/Important
0x002   SyncRoot        Native    5 min ago    ~/OneDrive
0x003   ActivelyOpen    Native    Just now     ~/Projects/app/src/main.rs
0x004   RecentlyAccessed Native   45 min ago   ~/Projects/app/Cargo.toml
...

# ═══════════════════════════════════════════════════════════════════
# PINNING DE PATHS
# ═══════════════════════════════════════════════════════════════════

# Fijar un path con maxima prioridad (no sera eviccionado)
$ lnxdrive watch pin ~/Documents/Critical
OK Pinned: ~/Documents/Critical

# Ver paths pinneados
$ lnxdrive watch pinned

# Quitar pin
$ lnxdrive watch unpin ~/Documents/Critical

# ═══════════════════════════════════════════════════════════════════
# REBALANCEO
# ═══════════════════════════════════════════════════════════════════

# Ver que pasaria sin ejecutar
$ lnxdrive watch rebalance --dry-run

# Ejecutar rebalanceo
$ lnxdrive watch rebalance

# ═══════════════════════════════════════════════════════════════════
# DIAGNOSTICOS
# ═══════════════════════════════════════════════════════════════════

# Verificar salud del sistema de watching
$ lnxdrive watch health
```

### 16.7 Metricas Prometheus

```prometheus
# HELP lnxdrive_file_watches_active Number of active file watches by priority
# TYPE lnxdrive_file_watches_active gauge
lnxdrive_file_watches_active{priority="pinned"} 45
lnxdrive_file_watches_active{priority="sync_root"} 12
lnxdrive_file_watches_active{priority="actively_open"} 23
lnxdrive_file_watches_active{priority="recently_accessed"} 3200
lnxdrive_file_watches_active{priority="moderately_accessed"} 1100
lnxdrive_file_watches_active{priority="stale"} 143

# HELP lnxdrive_file_watches_limit File watch limits
# TYPE lnxdrive_file_watches_limit gauge
lnxdrive_file_watches_limit{type="system"} 8192
lnxdrive_file_watches_limit{type="configured"} 6553
lnxdrive_file_watches_limit{type="available"} 2030

# HELP lnxdrive_file_watches_memory_bytes Estimated memory usage by file watches
# TYPE lnxdrive_file_watches_memory_bytes gauge
lnxdrive_file_watches_memory_bytes 4628480

# HELP lnxdrive_polling_paths_total Number of paths in polling mode
# TYPE lnxdrive_polling_paths_total gauge
lnxdrive_polling_paths_total 127

# HELP lnxdrive_file_events_total Total file events received
# TYPE lnxdrive_file_events_total counter
lnxdrive_file_events_total{type="modify",source="native"} 5432
lnxdrive_file_events_total{type="create",source="native"} 234
lnxdrive_file_events_total{type="delete",source="native"} 89
lnxdrive_file_events_total{type="modify",source="polling"} 156

# HELP lnxdrive_watch_evictions_total Total watch evictions performed
# TYPE lnxdrive_watch_evictions_total counter
lnxdrive_watch_evictions_total{reason="limit_reached"} 89
lnxdrive_watch_evictions_total{reason="rebalance"} 23
```

### 16.8 Flujo Completo de Deteccion de Cambios

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FLUJO DE DETECCION DE CAMBIOS                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  FUENTES DE EVENTOS                                                         │
│  ─────────────────                                                          │
│                                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                │
│  │   inotify    │     │   Polling    │     │    FUSE      │                │
│  │   Adapter    │     │   Worker     │     │   Events     │                │
│  │              │     │              │     │              │                │
│  │ IN_MODIFY    │     │ stat() loop  │     │ write()      │                │
│  │ IN_CREATE    │     │ mtime check  │     │ create()     │                │
│  │ IN_DELETE    │     │ hash check   │     │ unlink()     │                │
│  │ IN_MOVE      │     │              │     │ rename()     │                │
│  └──────┬───────┘     └──────┬───────┘     └──────┬───────┘                │
│         │                    │                    │                        │
│         └────────────────────┼────────────────────┘                        │
│                              │                                              │
│                              ▼                                              │
│  PROCESAMIENTO                                                              │
│  ─────────────                                                              │
│                    ┌─────────────────────┐                                  │
│                    │    Event Router     │                                  │
│                    │                     │                                  │
│                    │ * Normaliza eventos │                                  │
│                    │ * Debounce (100ms)  │                                  │
│                    │ * Coalesce (500ms)  │                                  │
│                    │ * Filtra ignorados  │                                  │
│                    └──────────┬──────────┘                                  │
│                               │                                             │
│                               ▼                                             │
│                    ┌─────────────────────┐                                  │
│                    │  Priority Manager   │                                  │
│                    │                     │                                  │
│                    │ * Actualiza LRU/LFU │                                  │
│                    │ * Ajusta prioridad  │                                  │
│                    │ * Trigger eviccion  │                                  │
│                    └──────────┬──────────┘                                  │
│                               │                                             │
│                               ▼                                             │
│  DISTRIBUCION                                                               │
│  ────────────                                                               │
│         ┌─────────────────────┼─────────────────────┐                      │
│         │                     │                     │                      │
│         ▼                     ▼                     ▼                      │
│  ┌────────────┐       ┌────────────┐       ┌────────────┐                  │
│  │ Sync Queue │       │ UI Notify  │       │  Metrics   │                  │
│  │            │       │            │       │            │                  │
│  │ -> SyncCore│       │ -> D-Bus   │       │ -> Prometheus                 │
│  │ -> Upload  │       │ -> Tray    │       │ -> Logging │                  │
│  │ -> Download│       │ -> Desktop │       │            │                  │
│  └────────────┘       └────────────┘       └────────────┘                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 16.9 Algoritmo de Decision: Native vs Polling

```
┌─────────────────────────────────────────────────────────────────────────────┐
│               ALGORITMO DE ASIGNACION DE WATCH                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  watch_request(path, priority)                                              │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────┐                                                      │
│  │ Path en lista    │──Yes──> return Ignored                               │
│  │   de ignorados?  │                                                      │
│  └────────┬─────────┘                                                      │
│           │ No                                                              │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │ Ya tiene watch?  │──Yes──> Actualizar prioridad si mayor                │
│  │                  │         return existing_handle                        │
│  └────────┬─────────┘                                                      │
│           │ No                                                              │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │Path en patrones  │──Yes──> Agregar a Polling                            │
│  │  always_poll?    │         return polling_handle                         │
│  └────────┬─────────┘                                                      │
│           │ No                                                              │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │Estrategia ==     │──Yes──> Agregar a Polling                            │
│  │    "polling"?    │         return polling_handle                         │
│  └────────┬─────────┘                                                      │
│           │ No                                                              │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │ Watches          │──Yes──> Agregar watch inotify                         │
│  │  disponibles?    │         Registrar en LRU/LFU                          │
│  │                  │         return native_handle                          │
│  └────────┬─────────┘                                                      │
│           │ No (limite alcanzado)                                           │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │Existe watch con  │                                                      │
│  │ prioridad < nueva│──No───> Agregar a Polling                            │
│  │  Y no pinneado?  │         return polling_handle                         │
│  └────────┬─────────┘                                                      │
│           │ Yes                                                             │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │ Seleccionar      │                                                      │
│  │ victima:         │                                                      │
│  │ - Min(prioridad) │                                                      │
│  │ - Min(LFU score) │                                                      │
│  │ - Max(age)       │                                                      │
│  └────────┬─────────┘                                                      │
│           │                                                                 │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │ Evictar victima: │                                                      │
│  │ - Remover inotify│                                                      │
│  │ - Agregar a poll │                                                      │
│  │ - Emitir metrica │                                                      │
│  └────────┬─────────┘                                                      │
│           │                                                                 │
│           ▼                                                                 │
│  ┌──────────────────┐                                                      │
│  │ Agregar nuevo    │                                                      │
│  │ watch inotify    │                                                      │
│  │ return handle    │                                                      │
│  └──────────────────┘                                                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 16.10 Consideraciones de Rendimiento y Limites

#### Calculo de Memoria

| Watches | Memoria Estimada | Notas |
|---------|------------------|-------|
| 1,000   | ~1 MB           | Instalacion pequena |
| 5,000   | ~5 MB           | Uso tipico |
| 10,000  | ~10 MB          | Repositorio mediano |
| 50,000  | ~50 MB          | Repositorio grande |
| 100,000 | ~100 MB         | Limite practico |

#### Comparativa de Estrategias

| Estrategia | Latencia | CPU | Memoria | Mejor para |
|------------|----------|-----|---------|------------|
| Native     | <10ms    | Minimo | ~1KB/watch | Directorios activos |
| Polling 5s | 5s max   | Medio | Minimo | Archivos grandes |
| Polling 60s| 60s max  | Bajo | Minimo | Archivos inactivos |
| Hybrid     | <10ms dirs | Medio | Balanceado | Directorios con muchos archivos |

#### Limites Recomendados

```yaml
# Laptop/Desktop tipico
file_watching:
  limits:
    system_reserve_percent: 20
    max_watches: 0  # auto
    min_watches: 1000

# Servidor/Workstation
file_watching:
  limits:
    system_reserve_percent: 10
    max_watches: 50000
    min_watches: 5000

# Sistema con recursos limitados
file_watching:
  limits:
    system_reserve_percent: 50
    max_watches: 2000
    min_watches: 500
  memory:
    hard_limit_mb: 20
```

---

## ⚠️ Riesgos y Mitigaciones

### B5: IN_Q_OVERFLOW (inotify Queue Overflow)

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | inotify kernel buffer, Event Router |
| **Simulación** | SIM-L3-004 |

**Descripción:**
Cuando el buffer del kernel para eventos inotify se llena más rápido de lo que la aplicación puede consumirlos, el kernel genera un evento `IN_Q_OVERFLOW` y descarta eventos. Esto causa pérdida silenciosa de cambios de archivos.

**Escenarios de Fallo:**
1. Build masivo genera miles de eventos en milisegundos
2. Copia o extracción de muchos archivos
3. Aplicación ocupada procesando evento largo mientras llegan más
4. Buffer del kernel configurado demasiado pequeño

**Mitigación Propuesta:**
```rust
impl EventRouter {
    pub fn handle_event(&mut self, event: InotifyEvent) -> Result<()> {
        match event.mask {
            EventMask::Q_OVERFLOW => {
                // Overflow detectado - lanzar full scan
                self.metrics.record_overflow();
                
                log::warn!("inotify queue overflow detected, initiating full scan");
                
                // Notificar al usuario
                self.notification_service.notify(Notification {
                    title: "Escaneando cambios",
                    body: "Se detectaron muchos cambios. Verificando consistencia...",
                    priority: Priority::Low,
                }).await?;
                
                // Encolar full scan
                self.trigger_full_scan(FullScanReason::QueueOverflow).await
            }
            _ => self.process_normal_event(event).await,
        }
    }
    
    async fn trigger_full_scan(&self, reason: FullScanReason) -> Result<()> {
        // Escanear todos los directorios observados
        for (path, _) in self.watched_paths.iter() {
            self.scan_directory_recursive(path).await?;
        }
        Ok(())
    }
}
```

**Tests Requeridos:**
- `test_inotify_overflow_triggers_scan`
- `test_full_scan_recovers_missed_changes`
- `test_bulk_file_operations_detected`

---

### C3: inotify Watch Eviction During Access

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | WatchManager, LRU/LFU eviction, Priority system |
| **Simulación** | SIM-L3-004 |

**Descripción:**
El algoritmo de evicción puede remover un watch de inotify de un directorio justo cuando un archivo dentro de ese directorio está siendo accedido frecuentemente, causando pérdida de eventos.

**Escenarios de Fallo:**
1. Usuario abre archivo, el directorio padre se evicta segundos después
2. Build compila en un directorio que perdió watch
3. Race entre actualización de prioridad y evicción

**Mitigación Propuesta:**
```rust
impl WatchManager {
    pub fn evict_for_new_watch(&mut self, new_priority: WatchPriority) -> Option<PathBuf> {
        // Bloquear evicción si hay acceso activo
        let candidates: Vec<_> = self.watches.iter()
            .filter(|(_, info)| {
                // No evictar si tiene handles abiertos
                if self.open_handles.contains(&info.path) {
                    return false;
                }
                
                // No evictar si acceso reciente (< 1 minuto)
                let elapsed = info.last_access.elapsed().unwrap_or_default();
                if elapsed < Duration::from_secs(60) {
                    return false;
                }
                
                // Solo evictar si prioridad menor
                info.priority < new_priority && !info.is_pinned
            })
            .collect();
        
        // Seleccionar víctima con menor score
        candidates.into_iter()
            .min_by_key(|(_, info)| {
                let freq_score = self.access_frequency.get(&info.handle)
                    .map(|s| s.score())
                    .unwrap_or(0.0);
                OrderedFloat(freq_score) // Menor score = primera víctima
            })
            .map(|(_, info)| {
                let path = info.path.clone();
                self.move_to_polling(&info.handle);
                path
            })
    }
}
```

**Tests Requeridos:**
- `test_active_file_prevents_dir_eviction`
- `test_recently_accessed_not_evicted`
- `test_eviction_grace_period`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)

---

## Ver tambien

- [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]] - Nucleo del sistema
- [[03-Arquitectura/01-arquitectura-hexagonal|Arquitectura Hexagonal]] - Puertos y adaptadores
- [[04-Componentes/12-auditoria|Auditoria]] - Registro de eventos de watching

