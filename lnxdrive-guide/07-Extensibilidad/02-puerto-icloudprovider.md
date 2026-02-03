# Puerto ICloudProvider

> **Ubicación:** `07-Extensibilidad/02-puerto-icloudprovider.md`
> **Relacionado:** [Arquitectura Multi-Proveedor](01-arquitectura-multi-proveedor.md), [Capas y Puertos](../03-Arquitectura/02-capas-y-puertos.md)

---

### 12.2 Puerto ICloudProvider

El puerto define las operaciones que todo proveedor debe implementar:

```rust
/// Puerto de salida para proveedores de almacenamiento en la nube.
/// Cada proveedor (OneDrive, Google Drive, etc.) implementa este trait.
#[async_trait]
pub trait ICloudProvider: Send + Sync {
    /// Identificador unico del proveedor
    fn provider_id(&self) -> &'static str;

    /// Nombre legible para UI
    fn display_name(&self) -> &'static str;

    /// Capacidades soportadas por este proveedor
    fn capabilities(&self) -> ProviderCapabilities;

    // ─── Autenticacion ───────────────────────────────────────────────

    /// Inicia el flujo de autenticacion OAuth2
    async fn authenticate(&self, flow: AuthFlow) -> Result<Tokens, AuthError>;

    /// Refresca tokens expirados
    async fn refresh_tokens(&self, refresh_token: &str) -> Result<Tokens, AuthError>;

    /// Revoca acceso (logout)
    async fn revoke(&self, tokens: &Tokens) -> Result<(), AuthError>;

    // ─── Sincronizacion Delta ────────────────────────────────────────

    /// Obtiene cambios desde el ultimo sync (delta/changes API)
    /// Si delta_token es None, retorna estado completo inicial
    async fn get_delta(
        &self,
        delta_token: Option<&str>,
    ) -> Result<DeltaResponse, SyncError>;

    /// Indica si el proveedor soporta delta sync nativo
    fn supports_native_delta(&self) -> bool;

    // ─── Operaciones de Archivo ──────────────────────────────────────

    /// Descarga contenido de un archivo
    async fn download_file(
        &self,
        remote_id: &str,
        range: Option<ByteRange>,
    ) -> Result<impl AsyncRead, TransferError>;

    /// Sube un archivo (< 4MB, upload simple)
    async fn upload_file_simple(
        &self,
        parent_id: &str,
        name: &str,
        content: impl AsyncRead,
        size: u64,
    ) -> Result<RemoteItem, TransferError>;

    /// Crea sesion de upload para archivos grandes
    async fn create_upload_session(
        &self,
        parent_id: &str,
        name: &str,
        size: u64,
    ) -> Result<UploadSession, TransferError>;

    /// Sube un chunk en una sesion existente
    async fn upload_chunk(
        &self,
        session: &UploadSession,
        chunk: &[u8],
        range: ByteRange,
    ) -> Result<UploadProgress, TransferError>;

    /// Obtiene metadata de un item
    async fn get_metadata(&self, remote_id: &str) -> Result<RemoteItem, SyncError>;

    /// Crea una carpeta
    async fn create_folder(
        &self,
        parent_id: &str,
        name: &str,
    ) -> Result<RemoteItem, SyncError>;

    /// Elimina un item (archivo o carpeta)
    async fn delete(&self, remote_id: &str) -> Result<(), SyncError>;

    /// Mueve/renombra un item
    async fn move_item(
        &self,
        item_id: &str,
        new_parent_id: &str,
        new_name: Option<&str>,
    ) -> Result<RemoteItem, SyncError>;

    // ─── Informacion de Cuenta ───────────────────────────────────────

    /// Obtiene informacion del usuario autenticado
    async fn get_user_info(&self) -> Result<UserInfo, AuthError>;

    /// Obtiene espacio disponible/usado
    async fn get_quota(&self) -> Result<QuotaInfo, SyncError>;
}

/// Capacidades que un proveedor puede o no soportar
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// Soporta delta/changes API para sync incremental eficiente
    pub native_delta: bool,

    /// Soporta uploads resumibles para archivos grandes
    pub resumable_uploads: bool,

    /// Tamaño maximo de archivo soportado
    pub max_file_size: Option<u64>,

    /// Soporta webhooks para notificaciones push
    pub webhooks: bool,

    /// Soporta compartir archivos/carpetas
    pub sharing: bool,

    /// Soporta versionado de archivos
    pub versioning: bool,

    /// Soporta papelera de reciclaje
    pub trash: bool,

    /// Limites de rate conocidos
    pub rate_limits: RateLimits,
}
```

### 12.3 Consideraciones por Proveedor

Cada proveedor tiene particularidades que el adaptador debe manejar:

| Proveedor | Delta API | Upload Grande | Particularidades |
|-----------|-----------|---------------|------------------|
| **OneDrive** | Nativo (`/delta`) | Sessions | Excelente para sync incremental |
| **Google Drive** | `changes.watch` | Resumable | Requiere polling o webhooks; pageToken en vez de deltaLink |
| **Dropbox** | `/list_folder/continue` | Sessions | API muy madura, cursor similar a delta |
| **Nextcloud** | WebDAV estandar | Chunked | Sin delta nativo; requiere comparar ETags |
| **pCloud** | `/diff` | Nativo | API propietaria pero completa |
| **Proton Drive** | Limitado | Encriptado | E2E encryption complica sync |

### 12.4 Implementacion de Adaptador: Google Drive (Ejemplo)

```rust
pub struct GoogleDriveProvider {
    client: GoogleDriveClient,
    config: GoogleDriveConfig,
}

#[async_trait]
impl ICloudProvider for GoogleDriveProvider {
    fn provider_id(&self) -> &'static str { "google-drive" }
    fn display_name(&self) -> &'static str { "Google Drive" }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_delta: false, // Usa changes API, no es delta puro
            resumable_uploads: true,
            max_file_size: Some(5 * 1024 * 1024 * 1024 * 1024), // 5TB
            webhooks: true,
            sharing: true,
            versioning: true,
            trash: true,
            rate_limits: RateLimits {
                requests_per_100_seconds: 20000,
                requests_per_100_seconds_per_user: 100,
            },
        }
    }

    async fn get_delta(&self, page_token: Option<&str>) -> Result<DeltaResponse, SyncError> {
        // Google Drive usa "changes" API con pageToken
        let response = if let Some(token) = page_token {
            self.client
                .changes()
                .list(token)
                .param("fields", "changes(file(id,name,mimeType,size,modifiedTime,trashed,parents)),nextPageToken,newStartPageToken")
                .doit()
                .await?
        } else {
            // Primera vez: obtener startPageToken
            let start = self.client.changes().get_start_page_token().doit().await?;
            return Ok(DeltaResponse::initial(start.start_page_token));
        };

        // Mapear respuesta de Google a modelo comun
        let items: Vec<DeltaItem> = response.changes
            .iter()
            .map(|change| self.map_change_to_delta_item(change))
            .collect();

        Ok(DeltaResponse {
            items,
            delta_token: response.new_start_page_token,
            has_more: response.next_page_token.is_some(),
        })
    }

    fn supports_native_delta(&self) -> bool { false }

    // ... resto de implementaciones
}
```

### 12.5 Registro de Proveedores

```rust
/// Registro central de proveedores disponibles
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn ICloudProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self { providers: HashMap::new() };

        // Registrar proveedores built-in
        registry.register(Arc::new(OneDriveProvider::new()));

        // Proveedores opcionales (feature flags en Cargo.toml)
        #[cfg(feature = "google-drive")]
        registry.register(Arc::new(GoogleDriveProvider::new()));

        #[cfg(feature = "dropbox")]
        registry.register(Arc::new(DropboxProvider::new()));

        #[cfg(feature = "nextcloud")]
        registry.register(Arc::new(NextcloudProvider::new()));

        registry
    }

    pub fn register(&mut self, provider: Arc<dyn ICloudProvider>) {
        self.providers.insert(provider.provider_id().to_string(), provider);
    }

    pub fn get(&self, provider_id: &str) -> Option<Arc<dyn ICloudProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}
```

---

## Ver tambien

- [Arquitectura Multi-Proveedor](01-arquitectura-multi-proveedor.md) - Vision general de extensibilidad
- [Multi-Cuenta con Namespaces](03-multi-cuenta-namespaces.md) - Soporte para multiples cuentas
- [Artefactos Reutilizables](04-artefactos-reutilizables.md) - Componentes extraibles como librerias
- [Capas y Puertos](../03-Arquitectura/02-capas-y-puertos.md) - Arquitectura hexagonal detallada
