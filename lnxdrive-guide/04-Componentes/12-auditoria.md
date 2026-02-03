# Auditoria y Explicabilidad

> **Ubicacion:** `04-Componentes/12-auditoria.md`
> **Relacionado:** [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]], [[01-Vision/02-principios-rectores|Principios Rectores]]

---

## Parte V.2: Auditoria y Explicabilidad

Cada accion genera una entrada de auditoria explicable:

```json
{
  "timestamp": "2026-01-28T15:30:45.123Z",
  "event_id": "evt_abc123",
  "action": "sync_skip",
  "item": {
    "local_path": "/home/user/OneDrive/document.docx",
    "remote_id": "ABC123!456",
    "state": "modified"
  },
  "result": "skipped",
  "reason": {
    "code": "REMOTE_MODIFIED_DURING_UPLOAD",
    "explanation": "El archivo fue modificado en OneDrive mientras se subia la version local. Se detecto un conflicto.",
    "details": {
      "local_hash": "sha256:abc...",
      "remote_hash": "sha256:def...",
      "local_modified": "2026-01-28T15:30:00Z",
      "remote_modified": "2026-01-28T15:30:30Z"
    }
  },
  "resolution": {
    "action_taken": "conflict_created",
    "conflict_id": "conf_xyz789",
    "suggested_strategies": ["keep-local", "keep-remote", "keep-both"]
  }
}
```

### Entidad AuditEntry

```
AuditEntry {
    timestamp: Timestamp
    action: Action
    item: SyncItem
    result: Result<Success, FailureReason>
    context: Map<String, String>
}
```

### Configuracion de Observabilidad

```yaml
# ~/.config/lnxdrive/config.yaml
observability:
  # Nivel de logging
  log_level: info           # trace | debug | info | warn | error

  # Archivo de log estructurado
  log_file: ~/.local/share/lnxdrive/lnxdrive.log
  log_format: json          # json | text

  # Retencion de auditoria
  audit_retention: 30d

  # Metricas Prometheus
  metrics:
    enabled: true
    endpoint: "127.0.0.1:9100"
```

### Metricas Prometheus

```prometheus
# HELP lnxdrive_files_total Total de archivos gestionados
# TYPE lnxdrive_files_total gauge
lnxdrive_files_total{state="online"} 1234
lnxdrive_files_total{state="hydrated"} 567
lnxdrive_files_total{state="modified"} 12
lnxdrive_files_total{state="conflicted"} 3

# HELP lnxdrive_sync_operations_total Operaciones de sincronizacion
# TYPE lnxdrive_sync_operations_total counter
lnxdrive_sync_operations_total{operation="upload",status="success"} 456
lnxdrive_sync_operations_total{operation="upload",status="failed"} 2
lnxdrive_sync_operations_total{operation="download",status="success"} 789
lnxdrive_sync_operations_total{operation="download",status="failed"} 1

# HELP lnxdrive_sync_bytes_total Bytes transferidos
# TYPE lnxdrive_sync_bytes_total counter
lnxdrive_sync_bytes_total{direction="upload"} 1234567890
lnxdrive_sync_bytes_total{direction="download"} 9876543210

# HELP lnxdrive_api_requests_total Llamadas a Microsoft Graph
# TYPE lnxdrive_api_requests_total counter
lnxdrive_api_requests_total{endpoint="delta",status="200"} 100
lnxdrive_api_requests_total{endpoint="delta",status="429"} 2

# HELP lnxdrive_conflicts_total Conflictos detectados
# TYPE lnxdrive_conflicts_total counter
lnxdrive_conflicts_total{resolution="auto"} 50
lnxdrive_conflicts_total{resolution="manual"} 5
lnxdrive_conflicts_total{resolution="pending"} 3

# HELP lnxdrive_hydration_duration_seconds Tiempo de hidratacion
# TYPE lnxdrive_hydration_duration_seconds histogram
lnxdrive_hydration_duration_seconds_bucket{le="1"} 500
lnxdrive_hydration_duration_seconds_bucket{le="5"} 800
lnxdrive_hydration_duration_seconds_bucket{le="30"} 950
lnxdrive_hydration_duration_seconds_bucket{le="+Inf"} 1000
```

### Puerto de Repositorio de Estado

```rust
trait IStateRepository {
    fn save_item(&self, item: &SyncItem) -> Result<()>;
    fn get_item(&self, id: &UniqueId) -> Result<Option<SyncItem>>;
    fn query_items(&self, filter: ItemFilter) -> Result<Vec<SyncItem>>;
    fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    fn get_audit_trail(&self, item_id: &UniqueId) -> Result<Vec<AuditEntry>>;
}
```

### Caso de Uso: Explicar Fallo

El caso de uso `ExplainFailureUseCase` genera explicaciones legibles de por que algo fallo:

```rust
// Casos de Uso (Interactors):
// - ExplainFailureUseCase -- Genera explicacion legible de por que algo fallo
```

### Principio de Explicabilidad

> "El usuario merece saber por que algo fallo"
> -- Principio Rector #3 de LNXDrive

A diferencia de otros clientes que muestran mensajes como "Error: sync failed", LNXDrive proporciona:

| Aspecto | Soluciones Actuales | LNXDrive |
|---------|---------------------|----------|
| **Explicabilidad** | "Error: sync failed" | "El archivo no se sincronizo porque el hash remoto cambio durante la subida. Se creo conflicto #xyz." |

### Codigos de Razon Comunes

| Codigo | Explicacion |
|--------|-------------|
| `REMOTE_MODIFIED_DURING_UPLOAD` | El archivo fue modificado en OneDrive mientras se subia la version local |
| `LOCAL_MODIFIED_DURING_DOWNLOAD` | El archivo fue modificado localmente mientras se descargaba |
| `NETWORK_TIMEOUT` | La conexion con el servidor expiro |
| `THROTTLING_EXCEEDED` | Se excedieron los limites de la API de Microsoft |
| `PERMISSION_DENIED` | Sin permisos para acceder al archivo |
| `FILE_TOO_LARGE` | El archivo excede el tamano maximo permitido |
| `PATH_TOO_LONG` | La ruta del archivo es demasiado larga |

---

## Ver tambien

- [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]] - Generacion de eventos de auditoria
- [[04-Componentes/11-conflictos|Resolucion de Conflictos]] - Auditoria de resoluciones
- [[01-Vision/02-principios-rectores|Principios Rectores]] - Explicabilidad como principio
- [[04-Componentes/09-rate-limiting|Rate Limiting]] - Metricas de throttling
