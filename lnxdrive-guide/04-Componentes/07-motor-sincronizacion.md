# Motor de Sincronizacion

> **Ubicacion:** `04-Componentes/07-motor-sincronizacion.md`
> **Relacionado:** [[03-Arquitectura/01-arquitectura-hexagonal|Arquitectura Hexagonal]], [[04-Componentes/08-microsoft-graph|Microsoft Graph]]

---

## Motor de Sincronizacion

El Motor de Sincronizacion es el corazon del nucleo de dominio (Core), completamente agnostico a tecnologia.

### Componentes del Motor

```
Motor de Sincronizacion
┌────────────────────────────────────────────────────────────────┐
│ * Delta Resolution * Conflict Detection * State Machine        │
│ * Hash Verification * Chunk Management * Retry Orchestration   │
└────────────────────────────────────────────────────────────────┘
```

### Entidades del Dominio

```
SyncItem {
    id: UniqueId
    local_path: Path
    remote_path: RemotePath
    state: ItemState
    hash: ContentHash
    last_sync: Timestamp
    metadata: Metadata
}

ItemState = Online | Hydrating | Hydrated | Modified | Conflicted | Error(reason)

Conflict {
    item: SyncItem
    local_version: Version
    remote_version: Version
    detected_at: Timestamp
    resolution: Option<Resolution>
}

AuditEntry {
    timestamp: Timestamp
    action: Action
    item: SyncItem
    result: Result<Success, FailureReason>
    context: Map<String, String>
}
```

### Casos de Uso (Interactors)

- `SyncFileUseCase` -- Sincroniza un archivo individual
- `ResolveConflictUseCase` -- Resuelve un conflicto con estrategia configurable
- `AuthenticateUseCase` -- Gestiona autenticacion OAuth2 PKCE
- `QueryDeltaUseCase` -- Obtiene cambios incrementales desde la nube
- `HydrateFileUseCase` -- Descarga contenido de archivo placeholder
- `DehydrateFileUseCase` -- Convierte archivo local en placeholder
- `ExplainFailureUseCase` -- Genera explicacion legible de por que algo fallo

### Motor de Estado (State Machine)

```
StateMachine para cada SyncItem:

    ┌──────────┐    access     ┌───────────┐   complete   ┌───────────┐
    │  Online  │ ────────────► │ Hydrating │ ───────────► │ Hydrated  │
    │(placeholder)│            │(downloading)│            │ (local)   │
    └──────────┘               └───────────┘              └───────────┘
         ▲                                                     │
         │                                                     │
         │    dehydrate                              modify    │
         └─────────────────────────────────────────────────────┘
                                    │
                                    ▼
                             ┌───────────┐
                             │ Modified  │ ──── conflict ───► Conflicted
                             │ (dirty)   │
                             └───────────┘
                                    │
                                    │ sync
                                    ▼
                             ┌───────────┐
                             │ Synced    │
                             └───────────┘
```

### Puertos de Entrada (Driving Ports)

```rust
trait ISyncController {
    fn start_sync(&self) -> Result<SyncSession>;
    fn pause_sync(&self) -> Result<()>;
    fn get_status(&self) -> SyncStatus;
    fn get_item_state(&self, path: &Path) -> ItemState;
    fn subscribe_events(&self, observer: &dyn IStateObserver);
}

trait IConflictResolver {
    fn list_conflicts(&self) -> Vec<Conflict>;
    fn resolve(&self, conflict_id: &ConflictId, strategy: ResolutionStrategy) -> Result<()>;
    fn preview_resolution(&self, conflict_id: &ConflictId, strategy: ResolutionStrategy) -> Preview;
}

trait IStateObserver {
    fn on_state_change(&self, item: &SyncItem, old: ItemState, new: ItemState);
    fn on_sync_progress(&self, progress: SyncProgress);
    fn on_error(&self, error: &SyncError, explanation: &str);
}
```

### Puertos de Salida (Driven Ports)

```rust
trait ICloudProvider {
    async fn authenticate(&self, flow: AuthFlow) -> Result<Tokens>;
    async fn refresh_tokens(&self, refresh_token: &str) -> Result<Tokens>;
    async fn get_delta(&self, delta_token: Option<&str>) -> Result<DeltaResponse>;
    async fn download_file(&self, remote_id: &str, range: Option<Range>) -> Result<Stream>;
    async fn upload_file(&self, path: &RemotePath, content: Stream) -> Result<RemoteItem>;
    async fn get_metadata(&self, remote_id: &str) -> Result<Metadata>;
}

trait ILocalFileSystem {
    fn create_placeholder(&self, path: &Path, metadata: &Metadata) -> Result<()>;
    fn hydrate(&self, path: &Path, content: Stream) -> Result<()>;
    fn dehydrate(&self, path: &Path) -> Result<()>;
    fn watch(&self, path: &Path, observer: &dyn IFileObserver) -> Result<WatchHandle>;
    fn get_state(&self, path: &Path) -> Result<FileSystemState>;
}

trait IStateRepository {
    fn save_item(&self, item: &SyncItem) -> Result<()>;
    fn get_item(&self, id: &UniqueId) -> Result<Option<SyncItem>>;
    fn query_items(&self, filter: ItemFilter) -> Result<Vec<SyncItem>>;
    fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    fn get_audit_trail(&self, item_id: &UniqueId) -> Result<Vec<AuditEntry>>;
}

trait INotificationService {
    fn notify(&self, notification: Notification) -> Result<()>;
    fn show_progress(&self, id: &str, progress: f64, message: &str) -> Result<()>;
}

trait IMetricsExporter {
    fn record_counter(&self, name: &str, value: u64, labels: &Labels);
    fn record_gauge(&self, name: &str, value: f64, labels: &Labels);
    fn record_histogram(&self, name: &str, value: f64, labels: &Labels);
}
```

---

## ⚠️ Riesgos y Mitigaciones

Esta sección documenta riesgos identificados durante la simulación arquitectónica y sus mitigaciones propuestas.

### B1: No Error Recovery Transitions

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P0 (Crítica) |
| **Componentes** | State Machine, SyncItem, Error handling |
| **Simulación** | SIM-L2-001 |

**Descripción:**
La máquina de estados actual no define transiciones de recuperación desde el estado `Error`. Un item que entra en `Error` podría quedarse bloqueado indefinidamente sin un camino claro de recuperación.

**Escenarios de Fallo:**
1. Error de red transitorio marca archivo como `Error`, no hay retry automático
2. Usuario corrige problema manualmente pero el sistema no detecta recuperación
3. Acumulación de items en estado `Error` sin mecanismo de limpieza

**Mitigación Propuesta:**
```rust
// Transiciones de recuperación desde Error
enum ErrorRecovery {
    AutoRetry { max_attempts: u32, backoff: Duration },
    ManualRetry,
    UserAction { action_type: UserActionType },
}

impl StateMachine {
    fn transition_from_error(&mut self, item: &mut SyncItem, trigger: ErrorRecoveryTrigger) -> Result<ItemState> {
        match (&item.state, trigger) {
            (ItemState::Error(reason), ErrorRecoveryTrigger::Retry) => {
                // Volver al estado anterior para reintentar
                Ok(item.previous_state.clone())
            }
            (ItemState::Error(_), ErrorRecoveryTrigger::UserFix) => {
                // Usuario confirmó corrección, reiniciar sync
                Ok(ItemState::Modified)
            }
            _ => Err(InvalidTransition)
        }
    }
}
```

**Tests Requeridos:**
- `test_error_to_hydrating_transition`
- `test_error_recovery_after_network_restore`
- `test_max_retry_exceeded_stays_error`

---

### A1: DBus Single Point of Failure

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | DBus Service, UI Adapters, Daemon |
| **Simulación** | SIM-L1-001, SIM-L1-002 |

**Descripción:**
Todos los adaptadores de UI (GNOME, KDE, Cosmic, CLI) dependen exclusivamente de DBus para comunicarse con el daemon. Si DBus falla, todas las interfaces quedan inoperativas aunque el daemon continúe funcionando.

**Escenarios de Fallo:**
1. DBus session daemon crash
2. Overflow de cola de mensajes DBus
3. Timeout del bus bajo carga
4. Colisión de nombres de servicio

**Mitigación Propuesta:**
```rust
pub struct DbusConnectionManager {
    connection: Option<Connection>,
    reconnect_attempts: AtomicU32,
    fallback_socket: Option<UnixSocket>,
}

impl DbusConnectionManager {
    pub async fn get_connection(&self) -> Result<&Connection> {
        if self.connection.is_none() || !self.connection.as_ref().unwrap().is_alive() {
            self.reconnect().await?;
        }
        self.connection.as_ref().ok_or(DbusError::NotConnected)
    }
    
    async fn reconnect(&mut self) -> Result<()> {
        for attempt in 0..MAX_RECONNECT_ATTEMPTS {
            match Connection::session().await {
                Ok(conn) => {
                    self.connection = Some(conn);
                    return Ok(());
                }
                Err(e) => {
                    tokio::time::sleep(backoff(attempt)).await;
                }
            }
        }
        Err(DbusError::ReconnectFailed)
    }
}
```

**Tests Requeridos:**
- `test_dbus_reconnect_after_crash`
- `test_daemon_continues_sync_without_dbus`
- `test_ui_recovers_after_dbus_restore`

---

### B4: Lifecycle Undefined (Resource Leaks)

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P3 (Baja) |
| **Componentes** | Observers, Handles, WatchHandles |
| **Simulación** | SIM-L2-005 |

**Descripción:**
No están definidos los ciclos de vida para observers, handles de archivo y watches. Esto puede causar memory leaks o recursos zombie cuando los componentes no se desregistran correctamente.

**Escenarios de Fallo:**
1. Observer registrado nunca se desregistra (UI cerrada sin cleanup)
2. WatchHandle perdido sin llamar a unwatch
3. Accumulation de callbacks en largo plazo

**Mitigación Propuesta:**
```rust
pub struct ObserverRegistry {
    observers: DashMap<ObserverId, WeakRef<dyn IStateObserver>>,
    gc_interval: Duration,
}

impl ObserverRegistry {
    pub fn register(&self, observer: Arc<dyn IStateObserver>) -> ObserverHandle {
        let id = ObserverId::new();
        self.observers.insert(id, Arc::downgrade(&observer));
        ObserverHandle { id, registry: self }
    }
    
    pub fn gc(&self) {
        self.observers.retain(|_, weak| weak.strong_count() > 0);
    }
}

impl Drop for ObserverHandle {
    fn drop(&mut self) {
        self.registry.observers.remove(&self.id);
    }
}
```

**Tests Requeridos:**
- `test_observer_auto_cleanup_on_drop`
- `test_weak_ref_gc_after_ui_close`
- `test_no_memory_leak_long_running`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)
> - [RISK-001-critical-paths.md](../.devtrail/02-design/risk-analysis/RISK-001-critical-paths.md)
>
> Diagramas de secuencia relacionados:
> - [SEQ-005-state-machine-transitions.puml](../.devtrail/02-design/diagrams/SEQ-005-state-machine-transitions.puml)
> - [SEQ-002-dbus-recovery.puml](../.devtrail/02-design/diagrams/SEQ-002-dbus-recovery.puml)

---

## Ver tambien

- [[03-Arquitectura/01-arquitectura-hexagonal|Arquitectura Hexagonal]] - Filosofia arquitectonica del sistema
- [[04-Componentes/08-microsoft-graph|Microsoft Graph]] - Integracion con la API de OneDrive
- [[04-Componentes/11-conflictos|Resolucion de Conflictos]] - Manejo visual de conflictos
- [[04-Componentes/12-auditoria|Auditoria]] - Sistema de trazabilidad

