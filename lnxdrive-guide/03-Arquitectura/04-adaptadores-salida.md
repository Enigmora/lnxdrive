# Adaptadores de Salida (Infraestructura)

> **Ubicación:** `03-Arquitectura/04-adaptadores-salida.md`
> **Relacionado:** [Capas y Puertos](02-capas-y-puertos.md), [Microsoft Graph](../04-Componentes/08-microsoft-graph.md)

---

## Resumen de Adaptadores

| Adaptador | Tecnología | Propósito |
|-----------|-----------|-----------|
| `msgraph-adapter` | Microsoft Graph API | Comunicación con OneDrive |
| `fuse-adapter` | libfuse3 / FUSE wrapper | Files-on-Demand (placeholders) |
| `sqlite-adapter` | SQLite | Persistencia de estado |
| `dbus-notify-adapter` | DBus org.freedesktop.Notifications | Notificaciones desktop |
| `prometheus-adapter` | Prometheus format | Métricas exportables |
| `gvfs-adapter` | GIO/GVfs | Integración GNOME Files |

## Detalle por Adaptador

### Microsoft Graph Adapter

Implementa el puerto `ICloudProvider` para OneDrive:

**Responsabilidades:**
- Autenticación OAuth2 + PKCE
- Delta sync (obtener cambios incrementales)
- Upload/download de archivos
- Manejo de rate limiting (429)
- Upload en chunks para archivos grandes

**Tecnologías:**
- `reqwest` para HTTP async
- `oauth2-rs` para flujos OAuth2
- Token bucket para rate limiting proactivo

### FUSE Adapter

Implementa el puerto `ILocalFileSystem` usando FUSE:

**Responsabilidades:**
- Crear y mantener placeholders
- Hidratación on-demand cuando se accede a archivos
- Extended attributes para metadata
- Comunicación con el core sobre accesos

**Tecnologías:**
- `fuser` (bindings libfuse3 para Rust)
- Extended attributes (xattr)

### SQLite Adapter

Implementa el puerto `IStateRepository`:

**Responsabilidades:**
- Persistir estado de cada SyncItem
- Almacenar audit trail
- Guardar delta tokens
- Cache de metadata
- Búsqueda full-text de archivos

**Tecnologías:**
- `sqlx` para acceso async a SQLite
- Migraciones embebidas
- Extensiones: FTS5, JSON1

#### Configuración de SQLite

```sql
-- Pragmas de rendimiento (aplicar al abrir conexión)
PRAGMA journal_mode = WAL;          -- Write-Ahead Logging para concurrencia
PRAGMA synchronous = NORMAL;        -- Balance entre seguridad y velocidad
PRAGMA cache_size = -64000;         -- 64MB de cache en memoria
PRAGMA temp_store = MEMORY;         -- Tablas temporales en RAM
PRAGMA mmap_size = 268435456;       -- 256MB memory-mapped I/O
PRAGMA foreign_keys = ON;           -- Integridad referencial
```

**¿Por qué WAL mode?**
- Permite lecturas concurrentes mientras se escribe
- El daemon puede consultar estado mientras sync escribe
- Mejor rendimiento para cargas mixtas read/write
- Checkpoints automáticos (o manuales con `PRAGMA wal_checkpoint`)

#### Esquema de Base de Datos

```sql
-- Tabla principal: estado de cada item sincronizado
CREATE TABLE sync_items (
    id TEXT PRIMARY KEY,                    -- ID de OneDrive (driveItem id)
    parent_id TEXT,                         -- ID del padre (para jerarquía)
    name TEXT NOT NULL,                     -- Nombre del archivo/carpeta
    path TEXT NOT NULL,                     -- Ruta completa local
    item_type TEXT NOT NULL,                -- 'file' | 'folder'
    state TEXT NOT NULL DEFAULT 'online',   -- Estado de sync (ver máquina de estados)

    -- Metadata de OneDrive
    etag TEXT,                              -- Para detectar cambios remotos
    ctag TEXT,                              -- Content tag (solo archivos)
    size INTEGER DEFAULT 0,                 -- Tamaño en bytes
    hash TEXT,                              -- SHA256 del contenido (quickXorHash de Graph)

    -- Timestamps
    remote_modified_at TEXT,                -- Última modificación en OneDrive (ISO8601)
    local_modified_at TEXT,                 -- Última modificación local
    last_sync_at TEXT,                      -- Último sync exitoso
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,

    -- Flags
    is_pinned INTEGER DEFAULT 0,            -- Mantener siempre hidratado
    is_excluded INTEGER DEFAULT 0,          -- Excluido de sync

    -- Metadata extendida (JSON)
    metadata TEXT,                          -- JSON con datos adicionales

    FOREIGN KEY (parent_id) REFERENCES sync_items(id) ON DELETE CASCADE
);

-- Índices para consultas frecuentes
CREATE INDEX idx_sync_items_parent ON sync_items(parent_id);
CREATE INDEX idx_sync_items_path ON sync_items(path);
CREATE INDEX idx_sync_items_state ON sync_items(state);
CREATE INDEX idx_sync_items_type_state ON sync_items(item_type, state);

-- FTS5: Búsqueda full-text de nombres de archivos
CREATE VIRTUAL TABLE sync_items_fts USING fts5(
    name,                                   -- Nombre del archivo
    path,                                   -- Ruta completa
    content=sync_items,                     -- Tabla fuente
    content_rowid=rowid                     -- Mapeo a sync_items
);

-- Triggers para mantener FTS sincronizado
CREATE TRIGGER sync_items_ai AFTER INSERT ON sync_items BEGIN
    INSERT INTO sync_items_fts(rowid, name, path) VALUES (NEW.rowid, NEW.name, NEW.path);
END;

CREATE TRIGGER sync_items_ad AFTER DELETE ON sync_items BEGIN
    INSERT INTO sync_items_fts(sync_items_fts, rowid, name, path)
    VALUES('delete', OLD.rowid, OLD.name, OLD.path);
END;

CREATE TRIGGER sync_items_au AFTER UPDATE ON sync_items BEGIN
    INSERT INTO sync_items_fts(sync_items_fts, rowid, name, path)
    VALUES('delete', OLD.rowid, OLD.name, OLD.path);
    INSERT INTO sync_items_fts(rowid, name, path) VALUES (NEW.rowid, NEW.name, NEW.path);
END;

-- Delta tokens para sync incremental
CREATE TABLE delta_tokens (
    account_id TEXT PRIMARY KEY,            -- Cuenta de OneDrive
    token TEXT NOT NULL,                    -- Delta link de Graph API
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Registro de conflictos
CREATE TABLE conflicts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    local_hash TEXT,
    remote_hash TEXT,
    local_modified_at TEXT,
    remote_modified_at TEXT,
    status TEXT DEFAULT 'pending',          -- 'pending' | 'resolved' | 'ignored'
    resolution TEXT,                        -- 'keep_local' | 'keep_remote' | 'keep_both'
    resolved_at TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (item_id) REFERENCES sync_items(id) ON DELETE CASCADE
);

CREATE INDEX idx_conflicts_status ON conflicts(status);
CREATE INDEX idx_conflicts_item ON conflicts(item_id);

-- Audit trail
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT,
    operation TEXT NOT NULL,                -- 'upload' | 'download' | 'delete' | 'rename' | 'conflict'
    old_state TEXT,
    new_state TEXT,
    details TEXT,                           -- JSON con detalles adicionales
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (item_id) REFERENCES sync_items(id) ON DELETE SET NULL
);

CREATE INDEX idx_audit_log_item ON audit_log(item_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);
```

#### Búsqueda Full-Text con FTS5

```rust
use sqlx::SqlitePool;

/// Buscar archivos por nombre usando FTS5
pub async fn search_files(pool: &SqlitePool, query: &str, limit: i32) -> Result<Vec<SyncItem>> {
    // FTS5 soporta operadores: AND, OR, NOT, NEAR, prefijos con *
    let results = sqlx::query_as!(
        SyncItem,
        r#"
        SELECT s.* FROM sync_items s
        INNER JOIN sync_items_fts fts ON s.rowid = fts.rowid
        WHERE sync_items_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
        query,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(results)
}

// Ejemplos de búsqueda:
// search_files(pool, "documento").await          // Búsqueda simple
// search_files(pool, "informe AND 2024").await   // Contiene ambos
// search_files(pool, "foto*").await              // Prefijo (foto, fotos, fotografía)
// search_files(pool, "src/components").await     // Búsqueda en path
```

#### Implementación del Adaptador

```rust
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

pub struct SqliteStateRepository {
    pool: SqlitePool,
}

impl SqliteStateRepository {
    pub async fn new(db_path: &Path) -> Result<Self> {
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    // Aplicar pragmas de rendimiento
                    sqlx::query("PRAGMA journal_mode = WAL").execute(conn).await?;
                    sqlx::query("PRAGMA synchronous = NORMAL").execute(conn).await?;
                    sqlx::query("PRAGMA cache_size = -64000").execute(conn).await?;
                    sqlx::query("PRAGMA temp_store = MEMORY").execute(conn).await?;
                    sqlx::query("PRAGMA mmap_size = 268435456").execute(conn).await?;
                    sqlx::query("PRAGMA foreign_keys = ON").execute(conn).await?;
                    Ok(())
                })
            })
            .connect(&database_url)
            .await?;

        // Ejecutar migraciones embebidas
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    /// Obtener items por estado (para UI de progreso)
    pub async fn get_items_by_state(&self, state: ItemState) -> Result<Vec<SyncItem>> {
        sqlx::query_as!(
            SyncItem,
            "SELECT * FROM sync_items WHERE state = ? ORDER BY path",
            state.as_str()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Obtener hijos de una carpeta (para FUSE readdir)
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<SyncItem>> {
        sqlx::query_as!(
            SyncItem,
            "SELECT * FROM sync_items WHERE parent_id = ? ORDER BY name",
            parent_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Checkpoint manual de WAL (llamar periódicamente o antes de backup)
    pub async fn checkpoint(&self) -> Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
```

#### Consideraciones de Rendimiento

| Operación | Índice usado | Latencia esperada |
|-----------|--------------|-------------------|
| Lookup por ID | PRIMARY KEY | <1ms |
| Listar hijos | idx_sync_items_parent | <5ms |
| Buscar por path | idx_sync_items_path | <1ms |
| Buscar por estado | idx_sync_items_state | <10ms |
| Full-text search | FTS5 | <50ms (100k archivos) |
| Insertar item | - | <1ms |

**Ubicación de la base de datos:**
```
~/.local/share/lnxdrive/state.db      # Base de datos principal
~/.local/share/lnxdrive/state.db-wal  # Write-Ahead Log
~/.local/share/lnxdrive/state.db-shm  # Shared memory (WAL index)
```

### DBus Notify Adapter

Implementa el puerto `INotificationService`:

**Responsabilidades:**
- Mostrar notificaciones de escritorio
- Progress notifications para operaciones largas
- Acciones en notificaciones (abrir conflicto, etc.)

**Tecnologías:**
- `zbus` para DBus async
- `org.freedesktop.Notifications` interface

### Prometheus Adapter

Implementa el puerto `IMetricsExporter`:

**Responsabilidades:**
- Exponer métricas en formato Prometheus
- Endpoint HTTP para scraping
- Métricas de sync, conflictos, rate limiting

**Métricas exportadas:**
```prometheus
lnxdrive_files_total{state="online"} 1234
lnxdrive_sync_operations_total{operation="upload",status="success"} 456
lnxdrive_api_requests_total{endpoint="delta",status="200"} 100
lnxdrive_conflicts_total{resolution="pending"} 3
```

### GVfs Adapter

Integración con GNOME Files:

**Responsabilidades:**
- Proveer overlay icons via GIO
- Columnas personalizadas en Nautilus
- Integración con GNOME Online Accounts

---

## Ver también

- [Capas y Puertos](02-capas-y-puertos.md) - Interfaces que implementan
- [Microsoft Graph](../04-Componentes/08-microsoft-graph.md) - Detalle del adaptador Graph
- [Files-on-Demand FUSE](../04-Componentes/01-files-on-demand-fuse.md) - Detalle del adaptador FUSE
- [Puerto ICloudProvider](../07-Extensibilidad/02-puerto-icloudprovider.md) - Interfaz para proveedores cloud
