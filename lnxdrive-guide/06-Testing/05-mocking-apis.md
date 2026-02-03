# Mocking de APIs Externas

> **Ubicación:** `06-Testing/05-mocking-apis.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

---

## 6. Mocking de APIs Externas

### 6.1 Mock de Microsoft Graph API

Para no depender de la API real durante desarrollo:

```rust
// src/adapters/graph/mock.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

/// Servidor mock de Microsoft Graph para testing
pub struct MockGraphServer {
    server: MockServer,
}

impl MockGraphServer {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Configura respuesta para delta API
    pub async fn mock_delta_response(&self, items: Vec<DriveItem>, delta_token: &str) {
        let response = serde_json::json!({
            "value": items,
            "@odata.deltaLink": format!("{}?token={}", self.url(), delta_token)
        });

        Mock::given(method("GET"))
            .and(path("/me/drive/root/delta"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&self.server)
            .await;
    }

    /// Configura respuesta de throttling (429)
    pub async fn mock_throttling(&self, retry_after: u32) {
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", retry_after.to_string())
            )
            .expect(1)  // Solo una vez, luego responde normal
            .mount(&self.server)
            .await;
    }

    /// Configura upload session
    pub async fn mock_upload_session(&self, upload_url: &str) {
        Mock::given(method("POST"))
            .and(path_regex(r"/me/drive/items/.*/createUploadSession"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uploadUrl": upload_url,
                "expirationDateTime": "2026-01-30T00:00:00Z"
            })))
            .mount(&self.server)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delta_sync_with_mock() {
        let mock = MockGraphServer::start().await;

        // Configurar respuesta mock
        mock.mock_delta_response(vec![
            DriveItem { id: "1".into(), name: "file1.txt".into(), .. },
            DriveItem { id: "2".into(), name: "file2.txt".into(), .. },
        ], "token123").await;

        // Crear cliente apuntando al mock
        let client = GraphClient::new(&mock.url(), "test-token");

        // Ejecutar sync
        let result = client.get_delta(None).await.unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.delta_token, Some("token123".into()));
    }

    #[tokio::test]
    async fn test_throttling_retry() {
        let mock = MockGraphServer::start().await;

        // Primera llamada: throttling
        mock.mock_throttling(5).await;

        // Segunda llamada: exito
        mock.mock_delta_response(vec![], "token").await;

        let client = GraphClient::new(&mock.url(), "test-token");

        // El cliente deberia reintentar automaticamente
        let result = client.get_delta(None).await.unwrap();

        // Verificar que funciono despues del retry
        assert!(result.items.is_empty());
    }
}
```

### 6.2 Mock de Sistema de Archivos Local

```rust
// src/adapters/filesystem/mock.rs

use tempfile::TempDir;

/// Filesystem temporal para testing
pub struct TestFileSystem {
    root: TempDir,
}

impl TestFileSystem {
    pub fn new() -> Self {
        Self { root: TempDir::new().unwrap() }
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// Crea estructura de archivos para testing
    pub fn setup_files(&self, files: &[(&str, &[u8])]) {
        for (path, content) in files {
            let full_path = self.root.path().join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(full_path, content).unwrap();
        }
    }

    /// Crea placeholder con xattrs
    pub fn setup_placeholder(&self, path: &str, remote_id: &str, size: u64) {
        let full_path = self.root.path().join(path);
        std::fs::write(&full_path, "").unwrap();  // Archivo vacio

        xattr::set(&full_path, "user.lnxdrive.state", b"online").unwrap();
        xattr::set(&full_path, "user.lnxdrive.remote_id", remote_id.as_bytes()).unwrap();
        xattr::set(&full_path, "user.lnxdrive.size", size.to_string().as_bytes()).unwrap();
    }

    /// Verifica estado de archivo
    pub fn assert_file_state(&self, path: &str, expected_state: &str) {
        let full_path = self.root.path().join(path);
        let state = xattr::get(&full_path, "user.lnxdrive.state")
            .unwrap()
            .unwrap();
        assert_eq!(String::from_utf8(state).unwrap(), expected_state);
    }
}
```

---

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de FUSE Filesystem](03-testing-fuse.md) - Mock de operaciones FUSE
- [Pipeline CI/CD](06-ci-cd-pipeline.md) - Servicios mock en CI
- [Automatizacion de Depuracion](08-automatizacion-depuracion.md) - Tests con captura de logs
