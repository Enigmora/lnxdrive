# Telemetría y Reporte de Errores

> **Ubicación:** `04-Componentes/13-telemetria.md`
> **Relacionado:** [Logging y Tracing](../06-Testing/07-logging-tracing.md), [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md)

---

## Parte XIII: Sistema de Telemetría

### 13.1 Principios Fundamentales

| Principio | Descripción |
|-----------|-------------|
| **Opt-in por defecto** | Desactivado hasta que el usuario lo habilite explícitamente |
| **Transparencia total** | El usuario puede ver exactamente qué datos se enviarán |
| **Control granular** | Configuración separada para crashes, errores, y estadísticas |
| **Anonimización** | Datos sensibles (rutas, nombres, archivos) se eliminan antes de enviar |
| **Resiliencia** | Fallos de telemetría no afectan la operación normal |

---

### 13.2 Arquitectura

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ARQUITECTURA DE TELEMETRÍA                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                       │
│  │   Daemon     │  │    FUSE      │  │   CLI/UI     │                       │
│  │  (tracing)   │  │  (tracing)   │  │  (tracing)   │                       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                       │
│         │                 │                 │                               │
│         │  Panic hooks + error collectors   │                               │
│         └────────────────┬┴─────────────────┘                               │
│                          ▼                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    ALMACENAMIENTO LOCAL                               │  │
│  │  ~/.local/share/lnxdrive/reports/                                     │  │
│  │                                                                       │  │
│  │  • crash-2026-01-31-abc123.json   (crash reports)                    │  │
│  │  • error-2026-01-31-def456.json   (errores no-fatales)               │  │
│  │  • system-info.json               (hardware, OS, versión)            │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│                  ┌───────────────────────────────────────┐                  │
│                  │    lnxdrive-telemetry                 │                  │
│                  │    (proceso separado)                 │                  │
│                  │                                       │                  │
│                  │  • Lee config de usuario              │                  │
│                  │  • Aplica anonimización               │                  │
│                  │  • Exporta vía OTLP/gRPC              │                  │
│                  │  • Resiliente a fallos de red         │                  │
│                  └───────────────────┬───────────────────┘                  │
│                                      │ (solo si opt-in)                     │
│                                      ▼                                      │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                 BACKEND OPENTELEMETRY (Google Cloud)                  │  │
│  │                                                                       │  │
│  │  Cloud Run ─► BigQuery + Cloud Trace + Error Reporting               │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### 13.3 Agente de Telemetría

El agente `lnxdrive-telemetry` es un **proceso independiente** que:

1. **No afecta al daemon**: Errores en telemetría no causan fallos en sincronización
2. **Resiliente**: Encola reportes si no hay conexión, reintenta con backoff
3. **Eficiente**: Bajo consumo de recursos, batch processing

```rust
// crates/lnxdrive-telemetry/src/main.rs

use opentelemetry_otlp::WithExportConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config = TelemetryConfig::load()?;
    
    // Salir inmediatamente si telemetría está deshabilitada
    if !config.enabled {
        return Ok(());
    }
    
    let collector = LocalReportCollector::new(&config.reports_dir)?;
    let anonymizer = Anonymizer::new(&config.anonymize);
    let exporter = OtlpExporter::new(&config.otlp_endpoint)?;
    
    loop {
        // Procesar reportes pendientes
        for report in collector.pending_reports()? {
            // 1. Anonimizar datos sensibles
            let anonymized = anonymizer.process(&report)?;
            
            // 2. Intentar enviar
            match exporter.export(&anonymized).await {
                Ok(_) => {
                    collector.mark_sent(&report.id)?;
                    tracing::info!(report_id = %report.id, "Report sent");
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "Failed to send, will retry");
                }
            }
        }
        
        tokio::time::sleep(config.check_interval).await;
    }
}
```

---

### 13.4 Generación de Reportes

#### Crash Reports (Panics)

```rust
// crates/lnxdrive-telemetry/src/panic_handler.rs

pub fn install_crash_reporter(reports_dir: &Path) {
    let reports_dir = reports_dir.to_path_buf();
    let default_hook = std::panic::take_hook();
    
    std::panic::set_hook(Box::new(move |panic_info| {
        let crash_report = CrashReport {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            component: std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned())),
            panic_message: panic_info.payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string()),
            location: panic_info.location().map(|l| format!("{}:{}:{}", 
                l.file(), l.line(), l.column())),
            backtrace: std::backtrace::Backtrace::capture().to_string(),
            os_info: OsInfo::collect(),
        };
        
        // Guardar localmente (nunca falla silenciosamente)
        if let Err(e) = save_crash_report(&reports_dir, &crash_report) {
            eprintln!("Failed to save crash report: {}", e);
        }
        
        // Llamar hook original
        default_hook(panic_info);
    }));
}

#[derive(Serialize)]
struct CrashReport {
    id: Uuid,
    timestamp: DateTime<Utc>,
    version: String,
    component: Option<String>,
    panic_message: Option<String>,
    location: Option<String>,
    backtrace: String,
    os_info: OsInfo,
}

#[derive(Serialize)]
struct OsInfo {
    os: String,           // "linux"
    kernel: String,       // "6.5.0"
    desktop: String,      // "GNOME 45"
    arch: String,         // "x86_64"
    // NO incluir: hostname, username, locale, timezone
}
```

#### Error Reports (Non-Fatal)

```rust
// crates/lnxdrive-telemetry/src/error_reporter.rs

pub struct ErrorReporter {
    reports_dir: PathBuf,
    enabled: bool,
}

impl ErrorReporter {
    pub fn report(&self, error: &impl std::error::Error, context: &str) {
        if !self.enabled {
            return;
        }
        
        // Solo reportar errores "interesantes" (no timeouts comunes, etc.)
        if !Self::should_report(error) {
            return;
        }
        
        let report = ErrorReport {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            error_type: std::any::type_name_of_val(error).to_string(),
            message: error.to_string(),
            context: context.to_string(),
            chain: Self::error_chain(error),
        };
        
        let _ = save_error_report(&self.reports_dir, &report);
    }
    
    fn should_report(error: &impl std::error::Error) -> bool {
        // No reportar errores comunes/esperados
        let msg = error.to_string().to_lowercase();
        !msg.contains("timeout") &&
        !msg.contains("connection refused") &&
        !msg.contains("rate limit")
    }
}
```

---

### 13.5 Anonimización

```rust
pub struct Anonymizer {
    strip_paths: bool,
    strip_usernames: bool,
    strip_filenames: bool,
}

impl Anonymizer {
    pub fn process(&self, report: &CrashReport) -> AnonymizedReport {
        let mut backtrace = report.backtrace.clone();
        
        if self.strip_paths {
            // /home/juan/OneDrive/documento.pdf -> <HOME>/OneDrive/<FILE>
            backtrace = Self::anonymize_paths(&backtrace);
        }
        
        if self.strip_usernames {
            // Reemplazar nombre de usuario del sistema
            if let Ok(user) = std::env::var("USER") {
                backtrace = backtrace.replace(&user, "<USER>");
            }
        }
        
        AnonymizedReport {
            id: report.id,
            timestamp: report.timestamp,
            version: report.version.clone(),
            component: report.component.clone(),
            panic_message: report.panic_message.clone(),
            // Línea/columna sí, pero archivo solo basename
            location: report.location.as_ref().map(|l| Self::anonymize_location(l)),
            backtrace,
            os_info: report.os_info.clone(),
        }
    }
    
    fn anonymize_paths(text: &str) -> String {
        let home = dirs::home_dir().unwrap_or_default();
        let home_str = home.to_string_lossy();
        
        text.replace(home_str.as_ref(), "<HOME>")
            .replace(r"\\/[^\\s/]+\\.[a-zA-Z0-9]+", "<FILE>")
    }
}
```

---

### 13.6 CLI: Gestión de Reportes

```bash
# Ver reportes pendientes
lnxdrive report list

# Salida:
# ID                                    Type    Date        Size
# ────────────────────────────────────────────────────────────────
# crash-abc123                          crash   2026-01-31  4.2 KB
# error-def456                          error   2026-01-31  1.1 KB
#
# Total: 2 reports pending (5.3 KB)

# Ver contenido de un reporte
lnxdrive report view crash-abc123

# Enviar un reporte específico
lnxdrive report send crash-abc123

# Enviar todos los reportes
lnxdrive report send --all

# Eliminar reportes sin enviar
lnxdrive report delete crash-abc123
lnxdrive report delete --all

# Configurar telemetría
lnxdrive config set telemetry.enabled true
lnxdrive config set telemetry.mode manual
```

---

### 13.7 Flujo de Consentimiento

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  [LNXDrive] Primer inicio                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ╔═══════════════════════════════════════════════════════════════════════╗  │
│  ║                                                                       ║  │
│  ║  ¿Deseas ayudar a mejorar LNXDrive?                                  ║  │
│  ║                                                                       ║  │
│  ║  Puedes compartir reportes de errores anonimizados cuando ocurran.   ║  │
│  ║  Esto nos ayuda a identificar y corregir problemas más rápido.       ║  │
│  ║                                                                       ║  │
│  ║  ✓ Los datos son anónimos (sin rutas personales ni archivos)         ║  │
│  ║  ✓ Puedes revisar cada reporte antes de enviarlo                     ║  │
│  ║  ✓ Puedes cambiar esta configuración en cualquier momento            ║  │
│  ║                                                                       ║  │
│  ║  [Ver ejemplo de reporte]              [Política de privacidad →]    ║  │
│  ║                                                                       ║  │
│  ║  ┌───────────────┐  ┌───────────────────┐  ┌───────────────────────┐ ║  │
│  ║  │  No, gracias  │  │ Preguntar siempre │  │ Sí, enviar automático │ ║  │
│  ║  └───────────────┘  └───────────────────┘  └───────────────────────┘ ║  │
│  ╚═══════════════════════════════════════════════════════════════════════╝  │
│                                                                             │
│  Configuración guardada en ~/.config/lnxdrive/config.yaml                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### 13.8 Relación con Otros Sistemas

| Sistema | Propósito | Diferencia con Telemetría |
|---------|-----------|---------------------------|
| **journald** | Logs locales del usuario | Nunca se envía remotamente |
| **tracing (Rust)** | Logging estructurado | Fuente de datos, no envío |
| **Prometheus** | Métricas operacionales | Solo local, scraping por usuario |
| **Telemetría** | Mejora del producto | Opt-in, anonimizado, remoto |

> [!WARNING]
> Las métricas Prometheus **NO** se envían al backend de telemetría.
> Prometheus es para observabilidad local del usuario.

---

### 13.9 Seguridad y Privacidad

#### Qué NO se recolecta nunca

| Dato | Razón |
|------|-------|
| Rutas absolutas | Pueden revelar estructura de archivos |
| Nombres de archivos | Pueden ser sensibles |
| Nombres de usuario | PII |
| Tokens OAuth | Secretos |
| Contenido de archivos | Datos del usuario |
| IPs completas | PII |
| Hostnames | Identificable |

#### Datos que SÍ se envían (si opt-in)

- Versión de LNXDrive
- Sistema operativo y versión del kernel
- Entorno de escritorio
- Stack trace anonimizado
- Mensaje de error (sin datos de usuario)

---

### 13.10 Integración con systemd

```ini
# ~/.config/systemd/user/lnxdrive-telemetry.service
[Unit]
Description=LNXDrive Telemetry Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=%h/.local/bin/lnxdrive-telemetry
Restart=on-failure
RestartSec=60

# Aislamiento: no afecta al daemon principal
Nice=19
IOSchedulingClass=idle
MemoryMax=50M

[Install]
WantedBy=default.target
```

---

## ⚠️ Riesgos y Mitigaciones

### T1: Exfiltración Accidental de Datos Sensibles

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P0 (Crítica) |
| **Componentes** | Anonymizer, CrashReport |

**Mitigación:**
- Anonimización obligatoria antes de cualquier envío
- Tests exhaustivos de regex de sanitización
- Modo "preview" para que usuario vea datos antes de enviar

**Tests Requeridos:**
- `test_paths_anonymized`
- `test_usernames_removed`
- `test_no_tokens_in_report`

---

### T2: Degradación de Rendimiento

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P1 (Alta) |
| **Componentes** | lnxdrive-telemetry process |

**Mitigación:**
- Proceso separado con `Nice=19` y `IOSchedulingClass=idle`
- Límite de memoria (`MemoryMax=50M`)
- Batch processing, no envío inmediato

---

> [!NOTE]
> Para la matriz completa de riesgos, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)

---

## Ver también

- [Logging y Tracing](../06-Testing/07-logging-tracing.md) - Sistema de logging local
- [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md) - Opciones de telemetría
- [CLI](06-cli.md) - Subcomando `report`
