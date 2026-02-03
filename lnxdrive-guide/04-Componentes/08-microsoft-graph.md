# Integracion con Microsoft Graph

> **Ubicacion:** `04-Componentes/08-microsoft-graph.md`
> **Relacionado:** [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]], [[04-Componentes/09-rate-limiting|Rate Limiting]]

---

## Parte VIII: Integracion con Microsoft Graph

### 8.1 Modelo de Autenticacion

Siguiendo las recomendaciones de la investigacion, implementamos el **modelo hibrido**:

```yaml
# Flujo de autenticacion
authentication:
  # App ID por defecto (embebido en el codigo)
  default_app_id: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

  # Usuario puede especificar su propio App ID
  custom_app_id: null  # o UUID del usuario

  # Flujo: OAuth2 Authorization Code + PKCE
  flow: authorization_code_pkce

  # Sin client_secret (app publica)
  client_secret: null

  # Scopes minimos necesarios
  scopes:
    - "Files.ReadWrite"
    - "offline_access"
    - "User.Read"

  # Redirect para desktop app
  redirect_uri: "http://127.0.0.1:PORT/callback"

  # Token storage seguro
  token_storage:
    backend: keyring  # keyring | file | custom
    service: "lnxdrive"
```

### 8.2 Sincronizacion Eficiente con Delta API

```rust
// Pseudocodigo del flujo de delta sync
async fn sync_delta(&self) -> Result<()> {
    // 1. Obtener delta token del ultimo sync
    let delta_token = self.state_repo.get_delta_token().await?;

    // 2. Llamar a /drive/root/delta
    let response = self.graph_client
        .get("/me/drive/root/delta")
        .query(&[("token", delta_token.as_deref().unwrap_or("latest"))])
        .send()
        .await?;

    // 3. Procesar cada cambio
    for item in response.value {
        match item.deleted {
            Some(_) => self.handle_remote_delete(&item).await?,
            None => self.handle_remote_change(&item).await?,
        }
    }

    // 4. Guardar nuevo delta token
    if let Some(new_token) = response.delta_link {
        self.state_repo.save_delta_token(&new_token).await?;
    }

    // 5. Si hay mas paginas, continuar
    if let Some(next_link) = response.next_link {
        self.sync_delta_page(&next_link).await?;
    }

    Ok(())
}
```

---

## ⚠️ Riesgos y Mitigaciones

Esta sección documenta riesgos identificados durante la simulación arquitectónica y sus mitigaciones propuestas.

### A3: Microsoft Graph Single Point of Failure

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | MS Graph API, OneDrive sync, Offline mode |
| **Simulación** | SIM-L3-003 |

**Descripción:**
Microsoft Graph es el único canal de comunicación con OneDrive. Cuando la API no está disponible, no puede ocurrir sincronización. Esto afecta a todas las cuentas OneDrive en configuración multi-proveedor.

**Escenarios de Fallo:**
1. Outage de Microsoft Graph (lado del servidor)
2. Rate limiting agresivo (respuestas 429)
3. Expiración de token OAuth sin refresh
4. Problemas de conectividad de red
5. Fallos de resolución DNS
6. Problemas de certificado TLS

**Mitigación Propuesta:**
```rust
pub struct OfflineQueue {
    pending_changes: VecDeque<PendingChange>,
    max_queue_size: usize,
}

pub struct GraphClientWithOffline {
    client: GraphClient,
    offline_queue: Mutex<OfflineQueue>,
    circuit_breaker: CircuitBreaker,
}

impl GraphClientWithOffline {
    pub async fn upload(&self, file: &SyncItem) -> Result<()> {
        if self.circuit_breaker.is_open() {
            // Encolar para cuando se restaure conexión
            self.offline_queue.lock().await.push(PendingChange::Upload(file.clone()));
            return Ok(());  // No bloquear al usuario
        }
        
        match self.client.upload(file).await {
            Ok(result) => {
                self.circuit_breaker.record_success();
                Ok(result)
            }
            Err(e) if e.is_transient() => {
                self.circuit_breaker.record_failure();
                self.offline_queue.lock().await.push(PendingChange::Upload(file.clone()));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    
    pub async fn drain_offline_queue(&self) -> Result<DrainResult> {
        // Llamar cuando se restaure conectividad
        let mut queue = self.offline_queue.lock().await;
        while let Some(change) = queue.pop_front() {
            self.process_pending_change(change).await?;
        }
        Ok(DrainResult::Complete)
    }
}
```

**Tests Requeridos:**
- `test_sync_continues_offline`
- `test_circuit_breaker_opens_after_failures`
- `test_queue_drains_on_reconnect`

---

### A4: Delta Token Expiry

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | Delta sync, IStateRepository, Full resync |
| **Simulación** | SIM-L3-002 |

**Descripción:**
El delta sync depende de un token que rastrea el último estado conocido. Este token expira después de 3-4 meses de inactividad o puede ser invalidado por Microsoft. Pérdida del delta token requiere una enumeración completa costosa.

**Escenarios de Fallo:**
1. Expiración del token (inactividad > 90 días)
2. Invalidación del token (lado del servidor)
3. Corrupción del token en almacenamiento local
4. API retorna respuesta 410 Gone

**Mitigación Propuesta:**
```rust
pub struct DeltaSyncManager {
    state_repo: Arc<dyn IStateRepository>,
    graph_client: Arc<GraphClient>,
    notification_service: Arc<dyn INotificationService>,
}

impl DeltaSyncManager {
    pub async fn sync_delta(&self) -> Result<DeltaResult> {
        let delta_token = self.state_repo.get_delta_token().await?;
        
        match self.graph_client.get_delta(delta_token.as_deref()).await {
            Ok(response) => {
                // Procesar cambios normalmente...
                self.process_delta_response(response).await
            }
            Err(GraphError::Gone410) => {
                // Token expirado o invalidado
                self.notification_service.notify(Notification {
                    title: "Resincronización necesaria",
                    body: "El token de sincronización expiró. Iniciando sincronización completa...",
                    priority: Priority::Normal,
                }).await?;
                
                // Fallback a sync completo con progreso
                self.full_resync_with_progress().await
            }
            Err(e) => Err(e.into()),
        }
    }
    
    async fn full_resync_with_progress(&self) -> Result<DeltaResult> {
        // Obtener delta desde el inicio (token = None)
        let mut page_count = 0;
        let mut total_items = 0;
        
        let response = self.graph_client.get_delta(None).await?;
        // ... procesar con indicador de progreso
        
        Ok(DeltaResult::FullResync { items_processed: total_items })
    }
}
```

**Tests Requeridos:**
- `test_410_triggers_full_resync`
- `test_resync_progress_notification`
- `test_delta_token_persisted_correctly`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)
> - [RISK-001-critical-paths.md](../.devtrail/02-design/risk-analysis/RISK-001-critical-paths.md)
>
> Diagrama de secuencia relacionado:
> - [SEQ-003-delta-token-expiry.puml](../.devtrail/02-design/diagrams/SEQ-003-delta-token-expiry.puml)

---

## Ver tambien

- [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]] - Nucleo del sistema de sincronizacion
- [[04-Componentes/09-rate-limiting|Rate Limiting]] - Estrategias de throttling
- [[03-Arquitectura/04-adaptadores-salida|Adaptadores de Salida]] - Implementacion de MS Graph adapter

