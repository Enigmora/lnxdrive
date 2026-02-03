# Estrategias de Rate Limiting

> **Ubicacion:** `04-Componentes/09-rate-limiting.md`
> **Relacionado:** [[04-Componentes/08-microsoft-graph|Microsoft Graph]], [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]]

---

## Parte VIII: Estrategias de Rate Limiting y Throttling (8.3)

El manejo de throttling en LNXDrive opera en **dos niveles complementarios**: reactivo (responder a errores 429) y proactivo (prevenir que ocurran).

### 8.3.1 Throttling Reactivo (Respuesta a 429)

```rust
async fn api_call_with_retry<T>(&self, request: Request) -> Result<T> {
    let mut attempts = 0;
    let max_attempts = 5;

    loop {
        let response = self.client.execute(request.clone()).await?;

        match response.status() {
            StatusCode::OK => return response.json().await,

            StatusCode::TOO_MANY_REQUESTS => {
                attempts += 1;
                if attempts >= max_attempts {
                    return Err(Error::ThrottlingExceeded);
                }

                // Respetar Retry-After header
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(30);

                // Log para auditoria
                self.audit.log(AuditEntry {
                    action: "api_throttled",
                    details: json!({
                        "attempt": attempts,
                        "retry_after_seconds": retry_after,
                    }),
                });

                // Exponential backoff con jitter
                let delay = Duration::from_secs(retry_after)
                    + Duration::from_millis(rand::random::<u64>() % 1000);

                tokio::time::sleep(delay).await;
            }

            status => return Err(Error::ApiError(status)),
        }
    }
}
```

### 8.3.2 Rate Limiting Proactivo

El rate limiting proactivo **previene** errores 429 controlando el flujo de requests antes de enviarlos:

```
┌─────────────────────────────────────────────────────────────────────┐
│  RATE LIMITER ADAPTATIVO                                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Niveles de control:                                                │
│                                                                     │
│  1. Token Bucket por endpoint:                                      │
│     * /delta: 10 req/min (polling frecuente)                        │
│     * /upload: 4 concurrentes, 60 req/min                           │
│     * /download: 8 concurrentes, sin limite estricto                │
│     * /metadata: 100 req/min (operaciones ligeras)                  │
│                                                                     │
│  2. Backpressure en cola de operaciones:                            │
│     * Cola priorizada (usuario explicito > automatico)              │
│     * Batch de metadatos antes de transferencias                    │
│     * Agrupacion de operaciones pequenas                            │
│                                                                     │
│  3. Adaptive throttling:                                            │
│     * Empieza conservador (50% del limite teorico)                  │
│     * Si 0 errores 429 en ventana de 5min -> aumenta 10%            │
│     * Si 1+ errores 429 -> reduce 50% por 10min                     │
│     * Limite maximo: 80% del teorico (margen de seguridad)          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Implementacion del Token Bucket:**

```rust
struct AdaptiveRateLimiter {
    buckets: HashMap<Endpoint, TokenBucket>,
    metrics: RateLimitMetrics,
    adaptation_window: Duration,
}

struct TokenBucket {
    capacity: u32,           // Tokens maximos
    tokens: AtomicU32,       // Tokens disponibles
    refill_rate: u32,        // Tokens por segundo
    last_refill: Instant,
    concurrent_limit: u32,   // Operaciones simultaneas
    active_operations: AtomicU32,
}

impl AdaptiveRateLimiter {
    async fn acquire(&self, endpoint: Endpoint) -> Result<RateLimitGuard> {
        let bucket = self.buckets.get(&endpoint)?;

        // Esperar si no hay tokens disponibles
        while bucket.tokens.load(Ordering::Relaxed) == 0 {
            bucket.refill();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Esperar si hay demasiadas operaciones concurrentes
        while bucket.active_operations.load(Ordering::Relaxed) >= bucket.concurrent_limit {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        bucket.tokens.fetch_sub(1, Ordering::Relaxed);
        bucket.active_operations.fetch_add(1, Ordering::Relaxed);

        Ok(RateLimitGuard { bucket })
    }

    fn on_throttle_response(&self, endpoint: Endpoint) {
        // Reducir limites adaptativamente
        if let Some(bucket) = self.buckets.get_mut(&endpoint) {
            bucket.capacity = (bucket.capacity as f32 * 0.5) as u32;
            bucket.concurrent_limit = (bucket.concurrent_limit as f32 * 0.5) as u32;
        }
        self.metrics.record_throttle(endpoint);
    }
}
```

### 8.3.3 Modo Bulk para Sincronizaciones Masivas

Cuando se detecta una sincronizacion inicial o masiva (>1000 archivos), el sistema entra en **modo bulk**:

```yaml
# Configuracion de modo bulk en config.yaml
sync:
  bulk_mode:
    threshold: 1000           # Archivos que activan modo bulk
    max_concurrent: 4         # Operaciones paralelas (vs 8 normal)
    requests_per_minute: 30   # Limite global conservador
    batch_metadata: true      # Agrupa operaciones de metadata
    priority: background      # Cede a operaciones interactivas del usuario

    # Estrategia de batching
    batching:
      metadata_batch_size: 50   # Consultar metadata en lotes
      upload_batch_size: 10     # Subir en lotes pequenos
      delay_between_batches: 2s # Pausa entre lotes
```

```
┌─────────────────────────────────────────────────────────────────┐
│  FLUJO DE SINCRONIZACION BULK                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. Deteccion: files_pending > bulk_threshold                   │
│                     │                                           │
│                     ▼                                           │
│  2. Activar modo bulk:                                          │
│     * Reducir concurrencia: 8 -> 4                              │
│     * Activar batching de metadata                              │
│     * Notificar UI: "Sincronizacion inicial en progreso"        │
│                     │                                           │
│                     ▼                                           │
│  3. Fase de metadata (prioridad):                               │
│     * Obtener delta completo                                    │
│     * Crear placeholders locales (rapido, sin descargas)        │
│     * Progress: "Indexando X de Y archivos..."                  │
│                     │                                           │
│                     ▼                                           │
│  4. Fase de transferencia (background):                         │
│     * Ordenar por prioridad (recientes primero)                 │
│     * Hidratar archivos pinned automaticamente                  │
│     * Resto: on-demand cuando usuario acceda                    │
│                     │                                           │
│                     ▼                                           │
│  5. Desactivar modo bulk cuando pending < threshold/2           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8.3.4 Archivos Gigantes (>100MB)

Los archivos grandes reciben tratamiento especial para evitar problemas de throttling y mejorar la resiliencia:

```
┌─────────────────────────────────────────────────────────────────┐
│  ESTRATEGIA PARA ARCHIVOS GRANDES                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Clasificacion por tamano:                                      │
│  |-- < 4MB:    PUT directo (1 request)                          │
│  |-- 4MB-100MB: Upload session, chunks de 10MB                  │
│  |-- 100MB-1GB: Upload session + rate limiting entre chunks     │
│  └-- > 1GB:     Upload session + prioridad exclusiva            │
│                                                                 │
│  Comportamiento para archivos >100MB:                           │
│  ────────────────────────────────────────────────────────────── │
│  * Solo UN upload gigante a la vez (no compite con otros)       │
│  * Chunks con pausa adaptativa entre ellos (500ms-2s)           │
│  * Resume automatico si se pierde conexion o hay error          │
│  * Prioridad baja por defecto (no bloquea archivos pequenos)    │
│  * Usuario puede elevar prioridad explicitamente                │
│                                                                 │
│  Nota sobre throttling en uploads grandes:                      │
│  ────────────────────────────────────────────────────────────── │
│  Microsoft Graph cuenta la SESION de upload, no cada chunk      │
│  como request separado. Por tanto, archivos gigantes causan     │
│  MENOS presion de throttling que muchos archivos pequenos.      │
│                                                                 │
│  El riesgo real es:                                             │
│  * Timeout de sesion (24 horas maximo)                          │
│  * Perdida de conexion durante transfer                         │
│  * Conflicto si el archivo remoto cambia durante upload         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

```yaml
# Configuracion de archivos grandes
sync:
  large_files:
    threshold: 100MB              # Umbral para tratamiento especial
    chunk_size: 10MB              # Tamano de chunk (multiplo de 320KB)
    max_concurrent_large: 1       # Solo 1 archivo gigante a la vez
    inter_chunk_delay: 500ms      # Pausa entre chunks
    priority: low                 # Prioridad por defecto
    resume_enabled: true          # Reanudar uploads interrumpidos

  giant_files:
    threshold: 1GB
    chunk_size: 20MB              # Chunks mas grandes para eficiencia
    inter_chunk_delay: 1s         # Mas pausa para no saturar
    schedule: off_peak            # Preferir horarios de baja actividad
    off_peak_hours: "02:00-06:00" # Ventana de sincronizacion nocturna
```

### 8.3.5 Metricas de Rate Limiting

```prometheus
# HELP lnxdrive_ratelimit_tokens_available Tokens disponibles por endpoint
# TYPE lnxdrive_ratelimit_tokens_available gauge
lnxdrive_ratelimit_tokens_available{endpoint="delta"} 8
lnxdrive_ratelimit_tokens_available{endpoint="upload"} 45
lnxdrive_ratelimit_tokens_available{endpoint="download"} 100

# HELP lnxdrive_ratelimit_queue_depth Operaciones en cola esperando
# TYPE lnxdrive_ratelimit_queue_depth gauge
lnxdrive_ratelimit_queue_depth{priority="user"} 0
lnxdrive_ratelimit_queue_depth{priority="background"} 23

# HELP lnxdrive_ratelimit_adaptive_factor Factor de adaptacion actual (1.0 = normal)
# TYPE lnxdrive_ratelimit_adaptive_factor gauge
lnxdrive_ratelimit_adaptive_factor{endpoint="upload"} 0.75

# HELP lnxdrive_bulk_mode_active Indica si el modo bulk esta activo
# TYPE lnxdrive_bulk_mode_active gauge
lnxdrive_bulk_mode_active 1
```

---

## ⚠️ Riesgos y Mitigaciones

### C6: Rate Limiter Race Condition

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P2 (Media) |
| **Componentes** | TokenBucket, AdaptiveRateLimiter |
| **Simulación** | SIM-L3-006 |

**Descripción:**
El token bucket usa operaciones atómicas para gestionar tokens, pero la secuencia check-then-act de `acquire()` puede causar sobre-asignación de tokens bajo alta concurrencia (más requests de los permitidos pasan simultáneamente).

**Escenarios de Fallo:**
1. Múltiples threads leen `tokens > 0` simultáneamente antes de decrementar
2. Race entre refill y consume
3. Contador de operaciones activas desincronizado

**Mitigación Propuesta:**
```rust
impl TokenBucket {
    pub fn try_acquire(&self) -> Option<TokenGuard> {
        // Operación atómica compare-and-swap
        loop {
            let current = self.tokens.load(Ordering::SeqCst);
            if current == 0 {
                return None;
            }
            
            // Intentar decrementar atómicamente
            if self.tokens.compare_exchange(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                // También verificar límite concurrente
                let active = self.active_operations.fetch_add(1, Ordering::SeqCst);
                if active >= self.concurrent_limit {
                    // Revertir si excede límite
                    self.tokens.fetch_add(1, Ordering::SeqCst);
                    self.active_operations.fetch_sub(1, Ordering::SeqCst);
                    return None;
                }
                
                return Some(TokenGuard { bucket: self });
            }
            // Si CAS falló, otro thread ganó, reintentar
        }
    }
}

impl Drop for TokenGuard {
    fn drop(&mut self) {
        self.bucket.active_operations.fetch_sub(1, Ordering::SeqCst);
    }
}
```

**Tests Requeridos:**
- `test_no_token_overallocation`
- `test_concurrent_acquire_under_stress`
- `test_cas_loop_fairness`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)

---

## Ver tambien

- [[04-Componentes/08-microsoft-graph|Microsoft Graph]] - Integracion con la API
- [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]] - Nucleo del sistema
- [[04-Componentes/12-auditoria|Auditoria]] - Registro de eventos de throttling

