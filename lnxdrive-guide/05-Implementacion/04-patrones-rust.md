# Patrones de Diseno en Rust

> **Ubicacion:** `05-Implementacion/04-patrones-rust.md`
> **Relacionado:** [Convenciones de Nomenclatura](03-convenciones-nomenclatura.md), [Stack Tecnologico](01-stack-tecnologico.md)

---

## 19.2 Patrones de Diseno Idiomaticos de Rust

### 19.2.1 Newtype Pattern (Seguridad de Tipos)

Envolver tipos primitivos para anadir semantica y prevenir errores:

```rust
use std::path::{Path, PathBuf};

/// Ruta validada para sincronizacion (debe ser absoluta)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncPath(PathBuf);

impl SyncPath {
    /// Crea una nueva SyncPath validando que sea absoluta
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SyncError> {
        let path = path.into();
        if path.is_absolute() {
            Ok(SyncPath(path))
        } else {
            Err(SyncError::RelativePath(path))
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

// Implementar AsRef para ergonomia
impl AsRef<Path> for SyncPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Hash de archivo (SHA-256)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileHash([u8; 32]);

impl FileHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        FileHash(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// ID unico de watch
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WatchHandle(u64);

impl WatchHandle {
    pub(crate) fn new(id: u64) -> Self {
        WatchHandle(id)
    }
}

// USO: Imposible confundir tipos
fn process_file(path: SyncPath, hash: FileHash, watch: WatchHandle) {
    // El compilador previene pasar argumentos en orden incorrecto
}
```

### 19.2.2 Builder Pattern (Configuracion Compleja)

Para tipos con muchos parametros opcionales:

```rust
/// Configuracion de sincronizacion
#[derive(Clone, Debug)]
pub struct SyncConfig {
    source: SyncPath,
    destination: SyncPath,
    buffer_size: usize,
    max_concurrent: usize,
    follow_symlinks: bool,
    exclude_patterns: Vec<String>,
}

/// Builder para SyncConfig
#[derive(Default)]
pub struct SyncConfigBuilder {
    source: Option<SyncPath>,
    destination: Option<SyncPath>,
    buffer_size: usize,
    max_concurrent: usize,
    follow_symlinks: bool,
    exclude_patterns: Vec<String>,
}

impl SyncConfigBuilder {
    pub fn new() -> Self {
        Self {
            source: None,
            destination: None,
            buffer_size: 8192,           // Default sensato
            max_concurrent: 4,            // Default sensato
            follow_symlinks: false,
            exclude_patterns: Vec::new(),
        }
    }

    /// Establece la ruta fuente (requerido)
    pub fn source(mut self, path: SyncPath) -> Self {
        self.source = Some(path);
        self
    }

    /// Establece la ruta destino (requerido)
    pub fn destination(mut self, path: SyncPath) -> Self {
        self.destination = Some(path);
        self
    }

    /// Tamano del buffer de lectura/escritura
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Maximo de operaciones concurrentes
    pub fn max_concurrent(mut self, n: usize) -> Self {
        self.max_concurrent = n;
        self
    }

    /// Si debe seguir enlaces simbolicos
    pub fn follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    /// Agrega un patron de exclusion
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Construye la configuracion validando campos requeridos
    pub fn build(self) -> Result<SyncConfig, SyncError> {
        let source = self.source.ok_or(SyncError::MissingField("source"))?;
        let destination = self.destination.ok_or(SyncError::MissingField("destination"))?;

        // Validacion: source y destination no pueden ser iguales
        if source == destination {
            return Err(SyncError::SameSourceDestination);
        }

        // Validacion: buffer_size debe ser > 0
        if self.buffer_size == 0 {
            return Err(SyncError::InvalidBufferSize);
        }

        Ok(SyncConfig {
            source,
            destination,
            buffer_size: self.buffer_size,
            max_concurrent: self.max_concurrent,
            follow_symlinks: self.follow_symlinks,
            exclude_patterns: self.exclude_patterns,
        })
    }
}

// USO
let config = SyncConfigBuilder::new()
    .source(SyncPath::new("/home/user/documents")?)
    .destination(SyncPath::new("/mnt/backup")?)
    .buffer_size(16384)
    .max_concurrent(8)
    .exclude("*.tmp")
    .exclude("node_modules")
    .build()?;
```

### 19.2.3 Type-State Pattern (Maquina de Estados en Compile-Time)

Garantiza transiciones de estado validas usando el sistema de tipos:

```rust
use std::marker::PhantomData;

// Estados como tipos (zero-sized)
pub struct Disconnected;
pub struct Authenticating;
pub struct Connected;
pub struct Syncing;

/// Cliente OneDrive con estado tipado
pub struct OneDriveClient<State> {
    config: ClientConfig,
    token: Option<AccessToken>,
    _state: PhantomData<State>,
}

impl OneDriveClient<Disconnected> {
    /// Crea un nuevo cliente desconectado
    pub fn new(config: ClientConfig) -> Self {
        OneDriveClient {
            config,
            token: None,
            _state: PhantomData,
        }
    }

    /// Inicia autenticacion - solo disponible en estado Disconnected
    pub async fn authenticate(self) -> Result<OneDriveClient<Authenticating>, AuthError> {
        // Iniciar flujo OAuth...
        Ok(OneDriveClient {
            config: self.config,
            token: None,
            _state: PhantomData,
        })
    }
}

impl OneDriveClient<Authenticating> {
    /// Completa autenticacion con codigo OAuth
    pub async fn complete_auth(
        self,
        auth_code: &str,
    ) -> Result<OneDriveClient<Connected>, AuthError> {
        let token = exchange_code_for_token(&self.config, auth_code).await?;

        Ok(OneDriveClient {
            config: self.config,
            token: Some(token),
            _state: PhantomData,
        })
    }

    /// Cancelar autenticacion
    pub fn cancel(self) -> OneDriveClient<Disconnected> {
        OneDriveClient {
            config: self.config,
            token: None,
            _state: PhantomData,
        }
    }
}

impl OneDriveClient<Connected> {
    /// Inicia sincronizacion - solo disponible en estado Connected
    pub async fn start_sync(self) -> Result<OneDriveClient<Syncing>, SyncError> {
        // Iniciar sync...
        Ok(OneDriveClient {
            config: self.config,
            token: self.token,
            _state: PhantomData,
        })
    }

    /// Desconectar
    pub async fn disconnect(self) -> OneDriveClient<Disconnected> {
        // Limpiar token...
        OneDriveClient {
            config: self.config,
            token: None,
            _state: PhantomData,
        }
    }

    /// Listar archivos - disponible cuando conectado
    pub async fn list_files(&self, path: &str) -> Result<Vec<FileEntry>, ApiError> {
        // API call...
        todo!()
    }
}

impl OneDriveClient<Syncing> {
    /// Detener sincronizacion
    pub async fn stop_sync(self) -> Result<OneDriveClient<Connected>, SyncError> {
        // Detener gracefully...
        Ok(OneDriveClient {
            config: self.config,
            token: self.token,
            _state: PhantomData,
        })
    }

    /// Obtener progreso - solo disponible durante sync
    pub fn progress(&self) -> SyncProgress {
        todo!()
    }
}

// USO: El compilador previene estados invalidos
async fn example() -> Result<(), Box<dyn std::error::Error>> {
    let client = OneDriveClient::new(config);

    // client.list_files("/").await?;  // ✗ ERROR: no disponible en Disconnected

    let client = client.authenticate().await?;
    let client = client.complete_auth("code123").await?;

    // Ahora si podemos listar
    let files = client.list_files("/").await?;  // ✓ OK

    let client = client.start_sync().await?;
    let progress = client.progress();  // ✓ OK: solo en Syncing

    // client.list_files("/").await?;  // ✗ ERROR: no disponible en Syncing

    let client = client.stop_sync().await?;
    let files = client.list_files("/").await?;  // ✓ OK: de vuelta en Connected

    Ok(())
}
```

### 19.2.4 RAII Pattern (Resource Acquisition Is Initialization)

Garantiza limpieza automatica de recursos:

```rust
use std::fs::File;
use std::path::{Path, PathBuf};

/// Guard para archivo de lock - se libera automaticamente
pub struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    /// Adquiere un lock creando archivo
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let lock_path = path.as_ref().to_path_buf();

        // Crear archivo de lock (falla si ya existe con O_EXCL)
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)  // Falla si existe
            .open(&lock_path)?;

        Ok(LockGuard { lock_path })
    }

    /// Intenta adquirir, retorna None si ya esta bloqueado
    pub fn try_acquire(path: impl AsRef<Path>) -> Option<Self> {
        Self::acquire(path).ok()
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Siempre limpiar el archivo de lock
        if let Err(e) = std::fs::remove_file(&self.lock_path) {
            eprintln!("Warning: failed to remove lock file: {}", e);
        }
    }
}

/// Guard para directorio temporal - se elimina al salir del scope
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Result<Self, std::io::Error> {
        let path = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path)?;
        Ok(TempDir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// USO: Recursos se limpian automaticamente
fn sync_with_lock() -> Result<(), SyncError> {
    // Lock se adquiere aqui
    let _lock = LockGuard::acquire("/tmp/lnxdrive.lock")?;

    // Directorio temporal para trabajo
    let temp = TempDir::new("lnxdrive-sync")?;

    // Hacer trabajo de sincronizacion...
    do_sync_work(temp.path())?;

    // Al salir del scope:
    // 1. temp se destruye → directorio eliminado
    // 2. _lock se destruye → archivo de lock eliminado
    Ok(())
}
```

## 19.3 Arquitectura Hexagonal en Rust

### Implementacion de Ports (Traits)

```rust
// ports/outbound/cloud_storage.rs

use async_trait::async_trait;
use crate::domain::models::{FileEntry, FileContent, CloudError};

/// Port para interaccion con almacenamiento en la nube
#[async_trait]
pub trait CloudStorage: Send + Sync {
    /// Lista archivos en una ruta remota
    async fn list_files(&self, path: &str) -> Result<Vec<FileEntry>, CloudError>;

    /// Descarga contenido de un archivo
    async fn download(&self, file_id: &str) -> Result<FileContent, CloudError>;

    /// Sube un archivo
    async fn upload(
        &self,
        parent_path: &str,
        name: &str,
        content: &[u8],
    ) -> Result<FileEntry, CloudError>;

    /// Elimina un archivo
    async fn delete(&self, file_id: &str) -> Result<(), CloudError>;

    /// Obtiene delta de cambios desde un token
    async fn get_delta(&self, delta_token: Option<&str>) -> Result<DeltaResponse, CloudError>;
}

/// Port para sistema de archivos local
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Lee contenido de archivo
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, FsError>;

    /// Escribe contenido a archivo
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<(), FsError>;

    /// Lista directorio
    async fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, FsError>;

    /// Obtiene metadata
    async fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError>;

    /// Crea directorio
    async fn create_dir(&self, path: &Path) -> Result<(), FsError>;

    /// Elimina archivo o directorio
    async fn remove(&self, path: &Path) -> Result<(), FsError>;
}

/// Port para cache de estado
#[async_trait]
pub trait StateCache: Send + Sync {
    /// Guarda estado de sincronizacion
    async fn save_state(&self, state: &SyncState) -> Result<(), CacheError>;

    /// Carga estado de sincronizacion
    async fn load_state(&self) -> Result<Option<SyncState>, CacheError>;

    /// Guarda entrada de archivo en cache
    async fn cache_entry(&self, entry: &FileEntry) -> Result<(), CacheError>;

    /// Obtiene entrada cacheada
    async fn get_cached(&self, path: &str) -> Result<Option<FileEntry>, CacheError>;
}

/// Port para bus de eventos
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publica un evento
    async fn publish(&self, event: SyncEvent) -> Result<(), EventError>;

    /// Suscribe a eventos
    fn subscribe(&self) -> EventReceiver;
}
```

### Implementacion de Adapters

```rust
// adapters/outbound/onedrive_api.rs

use async_trait::async_trait;
use crate::ports::outbound::CloudStorage;

pub struct OneDriveAdapter {
    client: reqwest::Client,
    token: Arc<RwLock<AccessToken>>,
    base_url: String,
}

impl OneDriveAdapter {
    pub fn new(token: AccessToken) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: Arc::new(RwLock::new(token)),
            base_url: "https://graph.microsoft.com/v1.0".to_string(),
        }
    }
}

#[async_trait]
impl CloudStorage for OneDriveAdapter {
    async fn list_files(&self, path: &str) -> Result<Vec<FileEntry>, CloudError> {
        let token = self.token.read().await;
        let url = format!("{}/me/drive/root:{}:/children", self.base_url, path);

        let response = self.client
            .get(&url)
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .json::<OneDriveListResponse>()
            .await?;

        Ok(response.value.into_iter().map(Into::into).collect())
    }

    async fn download(&self, file_id: &str) -> Result<FileContent, CloudError> {
        let token = self.token.read().await;
        let url = format!("{}/me/drive/items/{}/content", self.base_url, file_id);

        let bytes = self.client
            .get(&url)
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        Ok(FileContent::from(bytes))
    }

    // ... otros metodos
}
```

### Dependency Injection (Container)

```rust
// application/container.rs

use std::sync::Arc;
use crate::ports::outbound::*;
use crate::adapters::outbound::*;

/// Container para inyeccion de dependencias
pub struct Container {
    pub cloud_storage: Arc<dyn CloudStorage>,
    pub file_system: Arc<dyn FileSystem>,
    pub state_cache: Arc<dyn StateCache>,
    pub event_bus: Arc<dyn EventBus>,
}

impl Container {
    /// Crea container con implementaciones de produccion
    pub fn production(config: &Config) -> Result<Self, ContainerError> {
        let token = load_token_from_keyring()?;

        Ok(Self {
            cloud_storage: Arc::new(OneDriveAdapter::new(token)),
            file_system: Arc::new(LocalFileSystem::new(&config.sync_root)),
            state_cache: Arc::new(SqliteCache::new(&config.cache_path)?),
            event_bus: Arc::new(TokioEventBus::new()),
        })
    }

    /// Crea container con mocks para testing
    #[cfg(test)]
    pub fn test() -> Self {
        Self {
            cloud_storage: Arc::new(MockCloudStorage::new()),
            file_system: Arc::new(MockFileSystem::new()),
            state_cache: Arc::new(MockStateCache::new()),
            event_bus: Arc::new(MockEventBus::new()),
        }
    }
}
```

## 19.4 Manejo de Errores

### Estrategia Hibrida: thiserror + anyhow

```rust
// En crates de libreria (lnxdrive-core): usar thiserror
// domain/errors.rs

use thiserror::Error;
use std::path::PathBuf;

/// Errores del dominio de sincronizacion
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Archivo no encontrado: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Permiso denegado: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Conflicto detectado: {path} (local: {local_modified}, remote: {remote_modified})")]
    Conflict {
        path: PathBuf,
        local_modified: SystemTime,
        remote_modified: SystemTime,
    },

    #[error("Cuota excedida: {used}/{total} bytes")]
    QuotaExceeded { used: u64, total: u64 },

    #[error("Token expirado")]
    TokenExpired,

    #[error("Error de red: {message}")]
    NetworkError { message: String, #[source] source: Option<reqwest::Error> },

    #[error("Error de I/O")]
    IoError(#[from] std::io::Error),

    #[error("Error de API: {0}")]
    ApiError(#[from] ApiError),

    #[error("Error de cache: {0}")]
    CacheError(#[from] CacheError),
}

// En aplicaciones (lnxdrive-cli, lnxdrive-daemon): usar anyhow
// main.rs

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()
        .context("No se pudo cargar la configuracion")?;

    let container = Container::production(&config)
        .context("No se pudo inicializar el contenedor de dependencias")?;

    let coordinator = SyncCoordinator::new(&container);

    coordinator.sync()
        .await
        .context("La sincronizacion fallo")?;

    Ok(())
}
```

---

## Ver tambien

- [Convenciones de Nomenclatura](03-convenciones-nomenclatura.md) - Rust API Guidelines
- [Stack Tecnologico](01-stack-tecnologico.md) - Vision general del stack
- [Justificacion Rust](02-justificacion-rust.md) - Por que Rust
