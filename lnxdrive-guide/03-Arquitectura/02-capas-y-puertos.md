# Capas y Puertos

> **Ubicación:** `03-Arquitectura/02-capas-y-puertos.md`
> **Relacionado:** [Arquitectura Hexagonal](01-arquitectura-hexagonal.md), [Adaptadores de Entrada](03-adaptadores-entrada.md)

---

## Núcleo de Dominio (Core)

El corazón del sistema, **completamente agnóstico a tecnología**.

### Entidades

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

- `SyncFileUseCase` — Sincroniza un archivo individual
- `ResolveConflictUseCase` — Resuelve un conflicto con estrategia configurable
- `AuthenticateUseCase` — Gestiona autenticación OAuth2 PKCE
- `QueryDeltaUseCase` — Obtiene cambios incrementales desde la nube
- `HydrateFileUseCase` — Descarga contenido de archivo placeholder
- `DehydrateFileUseCase` — Convierte archivo local en placeholder
- `ExplainFailureUseCase` — Genera explicación legible de por qué algo falló

### Motor de Estado

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

## Puertos de Entrada (Driving Ports)

Interfaces que expone el core para ser consumido por adaptadores de UI:

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

## Puertos de Salida (Driven Ports)

Interfaces que el core necesita para comunicarse con infraestructura externa:

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

## Ver también

- [Arquitectura Hexagonal](01-arquitectura-hexagonal.md) - Visión general
- [Adaptadores de Entrada](03-adaptadores-entrada.md) - Implementaciones de UI
- [Adaptadores de Salida](04-adaptadores-salida.md) - Implementaciones de infraestructura
- [Puerto ICloudProvider](../07-Extensibilidad/02-puerto-icloudprovider.md) - Detalle del puerto de proveedores
