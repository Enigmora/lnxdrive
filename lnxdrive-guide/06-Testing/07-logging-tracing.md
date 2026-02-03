# Logging, Tracing y Obtencion de Registros

> **Ubicación:** `06-Testing/07-logging-tracing.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

---

## 9. Logging, Tracing y Obtencion de Registros para Depuracion

### 9.1 El Desafio del Logging en Sistemas Distribuidos

LNXDrive es un sistema con multiples componentes que se ejecutan en diferentes contextos:

```
+---------------------------------------------------------------------------+
|  COMPONENTES Y SUS CONTEXTOS DE LOGGING                                   |
+---------------------------------------------------------------------------+
|                                                                           |
|  +-------------+     +-------------+     +-------------+                  |
|  |  lnxdrive   |     |  lnxdrive   |     |  lnxdrive   |                  |
|  |   daemon    |     |    fuse     |     |    cli      |                  |
|  |             |     |             |     |             |                  |
|  | systemd     |     | proceso     |     | proceso     |                  |
|  | journal     |     | standalone  |     | efimero     |                  |
|  +------+------+     +------+------+     +------+------+                  |
|         |                   |                   |                         |
|         +-------------------+-------------------+                         |
|                             |                                             |
|                    +--------v--------+                                    |
|                    | Correlacion de  |                                    |
|                    |     eventos     |                                    |
|                    +--------+--------+                                    |
|                             |                                             |
|  +-------------+     +------v------+     +-------------+                  |
|  |  Nautilus   |     |   GNOME     |     |   DBus      |                  |
|  |  Extension  |     |   Shell     |     |  Messages   |                  |
|  |  (Python)   |     |  Extension  |     |             |                  |
|  |             |     |  (JS)       |     |             |                  |
|  | stdout/     |     | journalctl  |     | dbus-monitor|                  |
|  | syslog      |     | --user      |     |             |                  |
|  +-------------+     +-------------+     +-------------+                  |
|                                                                           |
+---------------------------------------------------------------------------+
```

**Problemas a resolver:**
1. Cada componente tiene su propio mecanismo de logging
2. Los logs no estan correlacionados (que evento en el daemon causo que en FUSE?)
3. En caso de crash, los logs pueden perderse
4. Debugging de componentes en containers/VMs requiere acceso remoto

### 9.2 Estrategia de Logging Unificada

#### 9.2.1 Formato de Log Estructurado

Todos los componentes Rust deben usar logging estructurado con `tracing`:

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"
tracing-journald = "0.3"  # Para integracion con systemd journal

// src/logging.rs
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging(config: &LogConfig) -> Result<LogGuard, LogError> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Capa para journald (produccion)
    let journald_layer = tracing_journald::layer()
        .ok()
        .filter(|_| config.use_journald);

    // Capa para archivo JSON (debugging detallado)
    let (file_writer, file_guard) = if let Some(ref path) = config.file_path {
        let file_appender = tracing_appender::rolling::daily(path, "lnxdrive.log");
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        (Some(writer), Some(guard))
    } else {
        (None, None)
    };

    let json_layer = file_writer.map(|w| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(w)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
    });

    // Capa para terminal (desarrollo)
    let terminal_layer = if config.use_terminal {
        Some(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true))
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(journald_layer)
        .with(json_layer)
        .with(terminal_layer)
        .init();

    Ok(LogGuard { _file_guard: file_guard })
}
```

#### 9.2.2 Contexto de Correlacion

Usar **trace IDs** para correlacionar eventos entre componentes:

```rust
use uuid::Uuid;

/// Contexto de operacion que se propaga entre componentes
#[derive(Clone, Debug)]
pub struct OperationContext {
    /// ID unico de la operacion (se propaga via DBus, headers, etc.)
    pub trace_id: Uuid,
    /// ID del span actual
    pub span_id: Uuid,
    /// Cuenta/namespace activo
    pub account_id: String,
    /// Archivo siendo procesado (si aplica)
    pub file_path: Option<PathBuf>,
}

impl OperationContext {
    pub fn new(account_id: &str) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            account_id: account_id.to_string(),
            file_path: None,
        }
    }

    /// Crear span de tracing con contexto
    pub fn span(&self, name: &str) -> tracing::Span {
        tracing::info_span!(
            "op",
            trace_id = %self.trace_id,
            span_id = %self.span_id,
            account = %self.account_id,
            file = ?self.file_path,
            op = name
        )
    }
}

// Uso en codigo
async fn sync_file(&self, ctx: &OperationContext, path: &Path) -> Result<()> {
    let span = ctx.span("sync_file").entered();

    tracing::info!(path = ?path, "Starting file sync");

    // La operacion...
    let result = self.upload(path).await;

    match &result {
        Ok(_) => tracing::info!("File sync completed successfully"),
        Err(e) => tracing::error!(error = ?e, "File sync failed"),
    }

    result
}
```

#### 9.2.3 Propagacion de Trace ID via DBus

```rust
// El daemon incluye trace_id en respuestas DBus
#[dbus_interface(name = "org.enigmora.LNXDrive.Sync")]
impl SyncService {
    async fn sync_file(&self, path: &str) -> fdo::Result<SyncResult> {
        let ctx = OperationContext::new(&self.current_account);
        let _span = ctx.span("dbus_sync_file").entered();

        tracing::info!(path = path, "DBus sync_file called");

        let result = self.core.sync_file(&ctx, Path::new(path)).await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(SyncResult {
            success: true,
            trace_id: ctx.trace_id.to_string(), // Cliente puede usar esto para buscar logs
        })
    }
}
```

### 9.3 Logging por Componente

#### 9.3.1 Daemon (systemd service)

```bash
# Ver logs en tiempo real
journalctl --user -u lnxdrive -f

# Ver logs con nivel debug
journalctl --user -u lnxdrive -p debug

# Filtrar por trace_id especifico
journalctl --user -u lnxdrive -o json | jq 'select(.TRACE_ID == "abc-123")'

# Ver logs de las ultimas 2 horas
journalctl --user -u lnxdrive --since "2 hours ago"

# Exportar logs para analisis
journalctl --user -u lnxdrive -o json > lnxdrive-logs.json
```

**Configuracion de systemd para logging mejorado:**

```ini
# ~/.config/systemd/user/lnxdrive.service
[Service]
# Logging estructurado a journald
StandardOutput=journal
StandardError=journal
SyslogIdentifier=lnxdrive

# Variables de entorno para logging
Environment=RUST_LOG=lnxdrive=debug,lnxdrive_fuse=debug,lnxdrive_graph=info
Environment=RUST_LOG_STYLE=always

# Campos adicionales en journal
LogExtraFields=LNXDRIVE_VERSION=0.1.0
LogExtraFields=LNXDRIVE_COMPONENT=daemon
```

#### 9.3.2 FUSE Filesystem

El proceso FUSE tiene consideraciones especiales porque puede colgarse:

```rust
// src/fuse/logging.rs

/// Logger especial para FUSE que escribe a archivo independiente
/// para preservar logs incluso si el proceso se cuelga
pub struct FuseLogger {
    file: std::sync::Mutex<std::fs::File>,
    buffer: std::sync::Mutex<Vec<LogEntry>>,
    flush_interval: Duration,
}

impl FuseLogger {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            file: std::sync::Mutex::new(file),
            buffer: std::sync::Mutex::new(Vec::with_capacity(100)),
            flush_interval: Duration::from_millis(500),
        })
    }

    /// Flush inmediato para operaciones criticas
    pub fn log_critical(&self, entry: LogEntry) {
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{}", serde_json::to_string(&entry).unwrap()).ok();
        file.flush().ok();
    }

    /// Log buffered para operaciones normales
    pub fn log(&self, entry: LogEntry) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(entry);

        if buffer.len() >= 100 {
            self.flush_buffer(&mut buffer);
        }
    }

    fn flush_buffer(&self, buffer: &mut Vec<LogEntry>) {
        if buffer.is_empty() {
            return;
        }

        let mut file = self.file.lock().unwrap();
        for entry in buffer.drain(..) {
            writeln!(file, "{}", serde_json::to_string(&entry).unwrap()).ok();
        }
        file.flush().ok();
    }
}
```

**Logs de FUSE en desarrollo:**

```bash
# El script de desarrollo ya configura logging
./scripts/dev-fuse-isolated.sh

# Los logs van a:
# - Terminal (RUST_LOG=debug)
# - Archivo: /tmp/lnxdrive-fuse-dev.XXXXXX/fuse.log

# Ver logs de FUSE en tiempo real
tail -f /tmp/lnxdrive-fuse-dev.*/fuse.log | jq .

# Filtrar operaciones de un archivo especifico
tail -f /tmp/lnxdrive-fuse-dev.*/fuse.log | jq 'select(.path | contains("documento.pdf"))'
```

#### 9.3.3 Extension de Nautilus (Python)

```python
# lnxdrive-nautilus.py
import logging
import sys
from gi.repository import GLib

# Configurar logging para extension de Nautilus
def setup_logging():
    # Crear logger especifico para LNXDrive
    logger = logging.getLogger('lnxdrive-nautilus')
    logger.setLevel(logging.DEBUG)

    # Handler para syslog (capturado por journald)
    from logging.handlers import SysLogHandler
    syslog_handler = SysLogHandler(address='/dev/log')
    syslog_handler.setFormatter(logging.Formatter(
        'lnxdrive-nautilus[%(process)d]: %(levelname)s %(message)s'
    ))
    logger.addHandler(syslog_handler)

    # Handler para archivo (debugging)
    file_handler = logging.FileHandler(
        GLib.get_user_cache_dir() + '/lnxdrive/nautilus-extension.log'
    )
    file_handler.setFormatter(logging.Formatter(
        '%(asctime)s [%(levelname)s] %(funcName)s: %(message)s'
    ))
    logger.addHandler(file_handler)

    return logger

logger = setup_logging()

class LNXDriveExtension(GObject.GObject, Nautilus.MenuProvider):
    def __init__(self):
        logger.info("LNXDrive Nautilus extension loaded")

    def get_file_items(self, files):
        logger.debug(f"get_file_items called with {len(files)} files")
        # ...
```

**Ver logs de extension Nautilus:**

```bash
# Logs van a journald via syslog
journalctl -f | grep lnxdrive-nautilus

# O al archivo de cache
tail -f ~/.cache/lnxdrive/nautilus-extension.log

# Reiniciar Nautilus para recargar extension y ver logs de inicio
nautilus -q && nautilus &
journalctl -f | grep -E "(nautilus|lnxdrive)"
```

#### 9.3.4 Extension de GNOME Shell (JavaScript)

```javascript
// extension.js
const ExtensionUtils = imports.misc.extensionUtils;
const Me = ExtensionUtils.getCurrentExtension();

// Logger personalizado para GNOME Shell
class LNXDriveLogger {
    constructor() {
        this.prefix = '[LNXDrive]';
    }

    _format(level, message, data) {
        const timestamp = new Date().toISOString();
        const dataStr = data ? ` ${JSON.stringify(data)}` : '';
        return `${timestamp} ${this.prefix} ${level}: ${message}${dataStr}`;
    }

    debug(message, data) {
        if (Me.metadata['debug-mode']) {
            log(this._format('DEBUG', message, data));
        }
    }

    info(message, data) {
        log(this._format('INFO', message, data));
    }

    warn(message, data) {
        log(this._format('WARN', message, data));
    }

    error(message, error) {
        logError(error, this._format('ERROR', message));
    }
}

const logger = new LNXDriveLogger();

// Uso
function init() {
    logger.info('Extension initializing', { version: Me.metadata.version });
}

function enable() {
    logger.info('Extension enabled');
    // ...
}
```

**Ver logs de GNOME Shell:**

```bash
# Logs de GNOME Shell van a journald
journalctl -f /usr/bin/gnome-shell

# Filtrar solo logs de LNXDrive
journalctl -f /usr/bin/gnome-shell | grep "\[LNXDrive\]"

# En sesion nested, los logs van a la terminal donde se lanzo
dbus-run-session -- gnome-shell --nested --wayland 2>&1 | grep "\[LNXDrive\]"

# Looking Glass (debugger interactivo de GNOME Shell)
# Presionar Alt+F2, escribir "lg", Enter
# En la pestana "Extensions" se pueden ver errores
```

### 9.4 Herramientas de Analisis de Logs

#### 9.4.1 Script de Agregacion de Logs

```bash
#!/bin/bash
# scripts/collect-logs.sh
# Recolecta logs de todos los componentes para analisis

OUTPUT_DIR="${1:-/tmp/lnxdrive-logs-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$OUTPUT_DIR"

echo "Recolectando logs en: $OUTPUT_DIR"

# 1. Logs del daemon (journald)
echo "  - Daemon logs..."
journalctl --user -u lnxdrive -o json --since "1 hour ago" \
    > "$OUTPUT_DIR/daemon.json" 2>/dev/null || echo "    (no disponible)"

# 2. Logs de FUSE
echo "  - FUSE logs..."
cp /tmp/lnxdrive-fuse-dev.*/fuse.log "$OUTPUT_DIR/fuse.log" 2>/dev/null \
    || echo "    (no disponible)"

# 3. Logs de extension Nautilus
echo "  - Nautilus extension logs..."
cp ~/.cache/lnxdrive/nautilus-extension.log "$OUTPUT_DIR/nautilus.log" 2>/dev/null \
    || echo "    (no disponible)"

# 4. Logs de GNOME Shell
echo "  - GNOME Shell logs..."
journalctl /usr/bin/gnome-shell -o json --since "1 hour ago" \
    | jq 'select(.MESSAGE | contains("[LNXDrive]"))' \
    > "$OUTPUT_DIR/gnome-shell.json" 2>/dev/null || echo "    (no disponible)"

# 5. Mensajes DBus recientes
echo "  - DBus messages..."
if command -v dbus-monitor &> /dev/null; then
    timeout 5 dbus-monitor --session "interface='org.enigmora.LNXDrive'" \
        > "$OUTPUT_DIR/dbus-messages.txt" 2>/dev/null &
fi

# 6. Estado del sistema
echo "  - System state..."
{
    echo "=== LNXDrive Processes ==="
    pgrep -af lnxdrive
    echo ""
    echo "=== Mount Points ==="
    mount | grep lnxdrive
    echo ""
    echo "=== systemd Status ==="
    systemctl --user status lnxdrive 2>/dev/null || echo "(not running)"
} > "$OUTPUT_DIR/system-state.txt"

# 7. Crear archivo combinado ordenado por tiempo
echo "  - Merging logs..."
python3 << 'EOF' - "$OUTPUT_DIR"
import json
import sys
from pathlib import Path
from datetime import datetime

output_dir = Path(sys.argv[1])
all_events = []

# Parsear daemon logs
daemon_log = output_dir / "daemon.json"
if daemon_log.exists():
    for line in daemon_log.read_text().splitlines():
        try:
            event = json.loads(line)
            all_events.append({
                "timestamp": event.get("__REALTIME_TIMESTAMP", "0"),
                "component": "daemon",
                "level": event.get("PRIORITY", "6"),
                "message": event.get("MESSAGE", ""),
                "trace_id": event.get("TRACE_ID", ""),
            })
        except:
            pass

# Parsear FUSE logs
fuse_log = output_dir / "fuse.log"
if fuse_log.exists():
    for line in fuse_log.read_text().splitlines():
        try:
            event = json.loads(line)
            all_events.append({
                "timestamp": event.get("timestamp", ""),
                "component": "fuse",
                "level": event.get("level", "INFO"),
                "message": event.get("message", ""),
                "trace_id": event.get("trace_id", ""),
            })
        except:
            pass

# Ordenar por timestamp
all_events.sort(key=lambda x: x["timestamp"])

# Escribir archivo combinado
with open(output_dir / "combined.json", "w") as f:
    for event in all_events:
        f.write(json.dumps(event) + "\n")

print(f"  Combined {len(all_events)} events")
EOF

echo ""
echo "Logs recolectados en: $OUTPUT_DIR"
echo "Archivos:"
ls -la "$OUTPUT_DIR"
```

#### 9.4.2 Visualizacion con jq

```bash
# Ver todos los errores
cat combined.json | jq 'select(.level == "ERROR" or .level == "3")'

# Seguir un trace_id especifico a traves de componentes
TRACE_ID="abc-123-def"
cat combined.json | jq --arg tid "$TRACE_ID" 'select(.trace_id == $tid)'

# Ver timeline de eventos para un archivo
cat combined.json | jq 'select(.message | contains("documento.pdf"))'

# Contar eventos por componente
cat combined.json | jq -r '.component' | sort | uniq -c

# Ver latencia de operaciones (si se registra duracion)
cat combined.json | jq 'select(.duration_ms != null) | {op: .message, ms: .duration_ms}' | sort -k2 -n
```

#### 9.4.3 Integracion con OpenTelemetry (Opcional, Avanzado)

Para debugging avanzado, se puede integrar con OpenTelemetry para tracing distribuido:

```rust
// Cargo.toml
[dependencies]
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
tracing-opentelemetry = "0.22"

// src/telemetry.rs
use opentelemetry::sdk::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;

pub fn init_telemetry(endpoint: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint)
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Anadir a la configuracion de tracing existente
    // ...

    Ok(())
}
```

```yaml
# docker-compose.yml para desarrollo con Jaeger
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # UI
      - "4317:4317"    # OTLP gRPC
    environment:
      COLLECTOR_OTLP_ENABLED: true

# Ver traces en http://localhost:16686
```

### 9.5 Debugging Remoto

#### 9.5.1 Debugging de Daemon en Container

```bash
# Construir con simbolos de debug
podman build -f docker/Containerfile.debug -t lnxdrive-debug .

# Ejecutar con debugger disponible
podman run -d --name lnxdrive-debug \
    --privileged \
    --systemd=always \
    -p 1234:1234 \
    lnxdrive-debug

# Conectar con lldb remotamente
lldb
(lldb) platform select remote-linux
(lldb) platform connect connect://localhost:1234
(lldb) attach --name lnxdrive-daemon
```

#### 9.5.2 Debugging en VM

```bash
# En la VM, iniciar gdbserver
ssh testuser@lnxdrive-test-vm "gdbserver :1234 /usr/local/bin/lnxdrive-daemon"

# En el host, conectar con gdb
rust-gdb target/debug/lnxdrive-daemon
(gdb) target remote lnxdrive-test-vm:1234
(gdb) continue
```

### 9.6 Preservacion de Logs en Crashes

#### 9.6.1 Panic Hook para Rust

```rust
// src/panic_handler.rs
use std::panic;
use std::fs::OpenOptions;
use std::io::Write;
use backtrace::Backtrace;

pub fn install_panic_handler() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        // Capturar backtrace
        let backtrace = Backtrace::new();

        // Escribir a archivo de crash
        let crash_log = format!(
            "/tmp/lnxdrive-crash-{}.log",
            std::process::id()
        );

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&crash_log)
        {
            writeln!(file, "=== LNXDrive Crash Report ===").ok();
            writeln!(file, "Time: {}", chrono::Utc::now()).ok();
            writeln!(file, "Panic: {}", panic_info).ok();
            writeln!(file, "\nBacktrace:\n{:?}", backtrace).ok();

            // Intentar volcar estado interno
            if let Some(state) = GLOBAL_STATE.try_read() {
                writeln!(file, "\nInternal State:\n{:#?}", *state).ok();
            }
        }

        eprintln!("Crash log written to: {}", crash_log);

        // Llamar al hook por defecto
        default_hook(panic_info);
    }));
}
```

#### 9.6.2 Coredumps Configurados

```bash
# Habilitar coredumps para LNXDrive
# ~/.config/systemd/user/lnxdrive.service
[Service]
LimitCORE=infinity

# Configurar ubicacion de coredumps
echo "/tmp/cores/core.%e.%p" | sudo tee /proc/sys/kernel/core_pattern
mkdir -p /tmp/cores

# Analizar coredump
coredumpctl list
coredumpctl debug lnxdrive-daemon
```

### 9.7 Configuracion de Logging por Entorno

```yaml
# ~/.config/lnxdrive/config.yaml
logging:
  # Nivel global
  level: info  # trace, debug, info, warn, error

  # Niveles por modulo
  modules:
    lnxdrive_core: debug
    lnxdrive_fuse: debug
    lnxdrive_graph: info
    hyper: warn  # Menos ruido de HTTP

  # Destinos
  outputs:
    # Terminal (solo desarrollo)
    terminal:
      enabled: true
      format: pretty  # pretty, json, compact
      colors: true

    # Archivo JSON (debugging)
    file:
      enabled: true
      path: ~/.local/share/lnxdrive/logs/
      rotation: daily
      retention: 7d
      format: json

    # journald (produccion)
    journald:
      enabled: true
      identifier: lnxdrive

    # OpenTelemetry (opcional)
    otlp:
      enabled: false
      endpoint: http://localhost:4317

  # Contexto adicional en todos los logs
  context:
    version: "0.1.0"
    instance_id: "${HOSTNAME}"
```

### 9.8 Checklist de Debugging

```bash
#!/bin/bash
# scripts/debug-checklist.sh
# Verificar que el entorno de debugging esta correctamente configurado

echo "=== LNXDrive Debug Environment Checklist ==="
echo ""

# 1. Variables de entorno
echo "1. Environment Variables:"
echo "   RUST_LOG=${RUST_LOG:-"(not set - will use config file)"}"
echo "   RUST_BACKTRACE=${RUST_BACKTRACE:-"(not set - recommend: 1)"}"
echo ""

# 2. Logging destinations
echo "2. Logging Destinations:"
echo -n "   journald: "
systemctl --user is-active lnxdrive &>/dev/null && echo "active" || echo "not running"
echo -n "   Log file: "
ls -la ~/.local/share/lnxdrive/logs/*.log 2>/dev/null | tail -1 || echo "(none)"
echo ""

# 3. Debug symbols
echo "3. Debug Symbols:"
echo -n "   Binary: "
file $(which lnxdrive-daemon 2>/dev/null || echo "target/debug/lnxdrive-daemon") 2>/dev/null \
    | grep -q "with debug_info" && echo "has debug symbols" || echo "stripped (rebuild with debug)"
echo ""

# 4. Coredumps
echo "4. Coredumps:"
echo -n "   Core pattern: "
cat /proc/sys/kernel/core_pattern
echo -n "   Core limit: "
ulimit -c
echo ""

# 5. Herramientas disponibles
echo "5. Debug Tools:"
echo -n "   gdb: "; command -v gdb &>/dev/null && echo "installed" || echo "NOT FOUND"
echo -n "   lldb: "; command -v lldb &>/dev/null && echo "installed" || echo "NOT FOUND"
echo -n "   jq: "; command -v jq &>/dev/null && echo "installed" || echo "NOT FOUND"
echo -n "   dbus-monitor: "; command -v dbus-monitor &>/dev/null && echo "installed" || echo "NOT FOUND"
echo ""

echo "=== Checklist Complete ==="
```

---

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de Servicios systemd](02-testing-systemd.md) - Logs del daemon con journald
- [Testing de FUSE Filesystem](03-testing-fuse.md) - Logging especial para FUSE
- [Testing de Extensiones Desktop](04-testing-desktop.md) - Logs de Nautilus y GNOME Shell
- [Automatizacion de Depuracion](08-automatizacion-depuracion.md) - Analisis automatizado de logs
