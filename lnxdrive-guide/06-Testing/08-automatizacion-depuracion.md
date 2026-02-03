# Automatizacion de Depuracion

> **Ubicación:** `06-Testing/08-automatizacion-depuracion.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

---

## 10. Automatizacion de Depuracion para Desarrollo Asistido

### 10.1 Filosofia: Debugging como Comando Simple

La depuracion en LNXDrive esta disenada para ser **invocable programaticamente** durante el ciclo de codificacion, no solo en CI/CD. Esto permite que:

1. **Desarrolladores** ejecuten pruebas y obtengan diagnosticos con un solo comando
2. **Agentes de IA** (Claude, Copilot, etc.) puedan probar cambios, obtener logs y analizar resultados
3. **Scripts de automatizacion** validen el codigo antes de commits

```
+---------------------------------------------------------------------------+
|  FLUJO DE DEPURACION AUTOMATIZADA                                         |
+---------------------------------------------------------------------------+
|                                                                           |
|   Desarrollador                    Agente de IA                           |
|        |                                |                                 |
|        v                                v                                 |
|   +-------------------------------------------------------------+         |
|   |           INTERFAZ UNIFICADA DE COMANDOS                    |         |
|   |                                                             |         |
|   |  make test-unit        -> Ejecutar tests unitarios          |         |
|   |  make test-component X -> Probar componente especifico      |         |
|   |  make diagnose         -> Obtener estado y logs             |         |
|   |  make validate         -> Validacion completa rapida        |         |
|   |                                                             |         |
|   +-------------------------------------------------------------+         |
|                              |                                            |
|                              v                                            |
|   +-------------------------------------------------------------+         |
|   |              SALIDA ESTRUCTURADA (JSON)                     |         |
|   |                                                             |         |
|   |  {                                                          |         |
|   |    "success": false,                                        |         |
|   |    "tests_run": 45,                                         |         |
|   |    "tests_failed": 2,                                       |         |
|   |    "failures": [...],                                       |         |
|   |    "logs": "base64...",                                     |         |
|   |    "suggestions": ["Check rate limiter config..."]          |         |
|   |  }                                                          |         |
|   |                                                             |         |
|   |  Parseabe por humanos Y por agentes de IA                   |         |
|   +-------------------------------------------------------------+         |
|                                                                           |
+---------------------------------------------------------------------------+
```

### 10.2 Comandos de Depuracion Unificados

#### 10.2.1 Makefile con Salida Estructurada

```makefile
# Makefile - Comandos de depuracion automatizada

# ==============================================================================
# COMANDOS PRINCIPALES PARA DESARROLLO Y AGENTES DE IA
# Todos los comandos producen salida JSON cuando se usa OUTPUT=json
# ==============================================================================

OUTPUT ?= human  # human | json

# --- Validacion Rapida (< 30 segundos) ----------------------------------------
# Usar antes de cada iteracion de codigo
.PHONY: validate
validate:
	@./scripts/dev-validate.sh --output $(OUTPUT)

# --- Tests por Componente -----------------------------------------------------
.PHONY: test-unit test-core test-fuse test-graph test-integration

test-unit:
	@./scripts/dev-test.sh unit --output $(OUTPUT)

test-core:
	@./scripts/dev-test.sh component core --output $(OUTPUT)

test-fuse:
	@./scripts/dev-test.sh component fuse --output $(OUTPUT)

test-graph:
	@./scripts/dev-test.sh component graph --output $(OUTPUT)

test-integration:
	@./scripts/dev-test.sh integration --output $(OUTPUT)

# --- Diagnostico del Sistema --------------------------------------------------
.PHONY: diagnose diagnose-quick

# Diagnostico completo con logs
diagnose:
	@./scripts/dev-diagnose.sh --output $(OUTPUT)

# Diagnostico rapido (solo estado, sin logs historicos)
diagnose-quick:
	@./scripts/dev-diagnose.sh --quick --output $(OUTPUT)

# --- Ejecucion con Captura de Logs --------------------------------------------
.PHONY: run-capture

# Ejecutar daemon capturando todos los logs para analisis
run-capture:
	@./scripts/dev-run-capture.sh --output $(OUTPUT)

# --- Analisis de Logs Existentes ----------------------------------------------
.PHONY: analyze-logs

analyze-logs:
	@./scripts/dev-analyze-logs.sh --output $(OUTPUT) $(FILTER)
```

#### 10.2.2 Script de Validacion Rapida

```bash
#!/bin/bash
# scripts/dev-validate.sh
# Validacion rapida para usar durante codificacion
# Disenado para ser invocado por desarrolladores o agentes de IA

set -e

OUTPUT_FORMAT="${OUTPUT:-human}"
RESULTS_FILE=$(mktemp)
START_TIME=$(date +%s.%N)

# Funcion para salida estructurada
output_json() {
    local success=$1
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $START_TIME" | bc)

    cat << EOF
{
    "command": "validate",
    "success": $success,
    "duration_seconds": $duration,
    "timestamp": "$(date -Iseconds)",
    "checks": $(cat "$RESULTS_FILE"),
    "summary": {
        "total": $(jq 'length' "$RESULTS_FILE"),
        "passed": $(jq '[.[] | select(.passed == true)] | length' "$RESULTS_FILE"),
        "failed": $(jq '[.[] | select(.passed == false)] | length' "$RESULTS_FILE")
    }
}
EOF
}

output_human() {
    local success=$1
    echo ""
    echo "==========================================================="
    if [ "$success" = "true" ]; then
        echo "VALIDACION EXITOSA"
    else
        echo "VALIDACION FALLIDA"
        echo ""
        echo "Fallos detectados:"
        jq -r '.[] | select(.passed == false) | "  - \(.name): \(.message)"' "$RESULTS_FILE"
    fi
    echo "==========================================================="
}

# Inicializar resultados
echo "[]" > "$RESULTS_FILE"

add_result() {
    local name=$1
    local passed=$2
    local message=$3
    local details=$4

    jq --arg name "$name" \
       --argjson passed "$passed" \
       --arg message "$message" \
       --arg details "$details" \
       '. += [{"name": $name, "passed": $passed, "message": $message, "details": $details}]' \
       "$RESULTS_FILE" > "${RESULTS_FILE}.tmp" && mv "${RESULTS_FILE}.tmp" "$RESULTS_FILE"
}

# --- Check 1: Formato de codigo -----------------------------------------------
echo -n "Checking code format... " >&2
if cargo fmt --check 2>/dev/null; then
    add_result "format" true "Code is properly formatted" ""
    echo "OK" >&2
else
    add_result "format" false "Code formatting issues found" "Run 'cargo fmt' to fix"
    echo "FAILED" >&2
fi

# --- Check 2: Linting ---------------------------------------------------------
echo -n "Running clippy... " >&2
CLIPPY_OUTPUT=$(cargo clippy --workspace --message-format=json 2>&1 || true)
CLIPPY_ERRORS=$(echo "$CLIPPY_OUTPUT" | jq -s '[.[] | select(.level == "error")] | length')
if [ "$CLIPPY_ERRORS" = "0" ]; then
    add_result "lint" true "No clippy errors" ""
    echo "OK" >&2
else
    add_result "lint" false "Clippy found $CLIPPY_ERRORS error(s)" "$CLIPPY_OUTPUT"
    echo "FAILED" >&2
fi

# --- Check 3: Compilacion -----------------------------------------------------
echo -n "Checking compilation... " >&2
if cargo check --workspace 2>/dev/null; then
    add_result "compile" true "Code compiles successfully" ""
    echo "OK" >&2
else
    COMPILE_OUTPUT=$(cargo check --workspace 2>&1 || true)
    add_result "compile" false "Compilation failed" "$COMPILE_OUTPUT"
    echo "FAILED" >&2
fi

# --- Check 4: Tests unitarios criticos ----------------------------------------
echo -n "Running critical unit tests... " >&2
TEST_OUTPUT=$(cargo test --workspace --lib -- --test-threads=4 2>&1 || true)
if echo "$TEST_OUTPUT" | grep -q "test result: ok"; then
    TESTS_PASSED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= passed)')
    add_result "unit_tests" true "$TESTS_PASSED tests passed" ""
    echo "OK" >&2
else
    FAILURES=$(echo "$TEST_OUTPUT" | grep -A5 "failures:" || echo "Unknown")
    add_result "unit_tests" false "Some tests failed" "$FAILURES"
    echo "FAILED" >&2
fi

# --- Determinar resultado final -----------------------------------------------
FAILED_COUNT=$(jq '[.[] | select(.passed == false)] | length' "$RESULTS_FILE")
if [ "$FAILED_COUNT" = "0" ]; then
    SUCCESS="true"
else
    SUCCESS="false"
fi

# --- Producir salida ----------------------------------------------------------
if [ "$OUTPUT_FORMAT" = "json" ]; then
    output_json "$SUCCESS"
else
    output_human "$SUCCESS"
fi

# Cleanup
rm -f "$RESULTS_FILE"

# Exit code
[ "$SUCCESS" = "true" ] && exit 0 || exit 1
```

#### 10.2.3 Script de Diagnostico

```bash
#!/bin/bash
# scripts/dev-diagnose.sh
# Obtiene estado completo del sistema y logs para analisis
# Disenado para ser parseado por agentes de IA

set -e

OUTPUT_FORMAT="${OUTPUT:-human}"
QUICK_MODE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --output) OUTPUT_FORMAT="$2"; shift 2 ;;
        --quick) QUICK_MODE=true; shift ;;
        *) shift ;;
    esac
done

collect_diagnostics() {
    local diag="{}"

    # 1. Estado de procesos
    local processes=$(pgrep -af "lnxdrive" 2>/dev/null | head -20 || echo "")
    diag=$(echo "$diag" | jq --arg p "$processes" '.processes = ($p | split("\n") | map(select(. != "")))')

    # 2. Estado de systemd
    local systemd_status=$(systemctl --user status lnxdrive 2>&1 || echo "not running")
    local systemd_active=$(systemctl --user is-active lnxdrive 2>/dev/null || echo "inactive")
    diag=$(echo "$diag" | jq --arg s "$systemd_status" --arg a "$systemd_active" \
        '.systemd = {"status": $s, "active": $a}')

    # 3. Puntos de montaje FUSE
    local mounts=$(mount | grep -E "(lnxdrive|fuse)" 2>/dev/null || echo "none")
    diag=$(echo "$diag" | jq --arg m "$mounts" '.fuse_mounts = $m')

    # 4. DBus service
    local dbus_active=$(dbus-send --session --dest=org.freedesktop.DBus \
        --type=method_call --print-reply /org/freedesktop/DBus \
        org.freedesktop.DBus.ListNames 2>/dev/null | grep -c "org.enigmora.LNXDrive" || echo "0")
    diag=$(echo "$diag" | jq --argjson d "$dbus_active" '.dbus_service_active = ($d > 0)')

    # 5. Logs recientes (si no es modo rapido)
    if [ "$QUICK_MODE" = false ]; then
        local recent_logs=$(journalctl --user -u lnxdrive --since "5 minutes ago" \
            -o json 2>/dev/null | jq -s '.' || echo "[]")
        diag=$(echo "$diag" | jq --argjson l "$recent_logs" '.recent_logs = $l')

        local error_logs=$(journalctl --user -u lnxdrive --since "1 hour ago" \
            -p err -o json 2>/dev/null | jq -s '.' || echo "[]")
        diag=$(echo "$diag" | jq --argjson e "$error_logs" '.error_logs = $e')
    fi

    # 6. Archivos de configuracion
    local config_exists=$([ -f ~/.config/lnxdrive/config.yaml ] && echo "true" || echo "false")
    diag=$(echo "$diag" | jq --argjson c "$config_exists" '.config_file_exists = $c')

    # 7. Espacio en disco
    local disk_usage=$(df -h ~ | tail -1 | awk '{print $5}' | tr -d '%')
    diag=$(echo "$diag" | jq --argjson d "$disk_usage" '.disk_usage_percent = $d')

    # 8. Resumen de salud
    local health="healthy"
    local issues=()

    [ "$systemd_active" != "active" ] && health="degraded" && issues+=("daemon not running")
    [ "$dbus_active" = "0" ] && health="degraded" && issues+=("dbus service not responding")
    [ "$disk_usage" -gt 90 ] && health="warning" && issues+=("disk usage above 90%")

    diag=$(echo "$diag" | jq --arg h "$health" '.health = $h')
    diag=$(echo "$diag" | jq --argjson i "$(printf '%s\n' "${issues[@]}" | jq -R . | jq -s .)" '.issues = $i')

    echo "$diag"
}

# Recolectar diagnosticos
DIAGNOSTICS=$(collect_diagnostics)

# Salida
if [ "$OUTPUT_FORMAT" = "json" ]; then
    echo "$DIAGNOSTICS" | jq '{
        "command": "diagnose",
        "timestamp": (now | todate),
        "quick_mode": '"$QUICK_MODE"',
        "diagnostics": .
    }'
else
    echo "==========================================================="
    echo "         DIAGNOSTICO DE LNXDRIVE"
    echo "==========================================================="
    echo ""
    echo "Estado: $(echo "$DIAGNOSTICS" | jq -r '.health')"
    echo ""

    issues=$(echo "$DIAGNOSTICS" | jq -r '.issues[]' 2>/dev/null)
    if [ -n "$issues" ]; then
        echo "Problemas detectados:"
        echo "$issues" | while read -r issue; do
            echo "  ! $issue"
        done
        echo ""
    fi

    echo "Daemon: $(echo "$DIAGNOSTICS" | jq -r '.systemd.active')"
    echo "DBus: $(echo "$DIAGNOSTICS" | jq -r 'if .dbus_service_active then "active" else "inactive" end')"
    echo "FUSE: $(echo "$DIAGNOSTICS" | jq -r '.fuse_mounts')"
    echo "Disco: $(echo "$DIAGNOSTICS" | jq -r '.disk_usage_percent')% usado"
    echo ""

    if [ "$QUICK_MODE" = false ]; then
        error_count=$(echo "$DIAGNOSTICS" | jq '.error_logs | length')
        if [ "$error_count" -gt 0 ]; then
            echo "Errores recientes ($error_count):"
            echo "$DIAGNOSTICS" | jq -r '.error_logs[-5:][] | "  [\(.PRIORITY)] \(.MESSAGE)"' 2>/dev/null
        fi
    fi
    echo "==========================================================="
fi
```

#### 10.2.4 Script de Tests con Captura de Logs

```bash
#!/bin/bash
# scripts/dev-test.sh
# Ejecuta tests capturando logs para analisis posterior
# Uso: ./dev-test.sh [unit|integration|component <name>] --output [human|json]

set -e

TEST_TYPE="${1:-unit}"
COMPONENT="${2:-}"
OUTPUT_FORMAT="${OUTPUT:-human}"
LOG_DIR=$(mktemp -d)
RESULTS="{}"

cleanup() {
    rm -rf "$LOG_DIR"
}
trap cleanup EXIT

run_tests() {
    local test_cmd=""
    local test_name=""

    case "$TEST_TYPE" in
        unit)
            test_cmd="cargo test --workspace --lib"
            test_name="Unit Tests"
            ;;
        integration)
            test_cmd="cargo test --workspace --test '*'"
            test_name="Integration Tests"
            ;;
        component)
            if [ -z "$COMPONENT" ]; then
                echo "Error: component name required" >&2
                exit 1
            fi
            test_cmd="cargo test -p lnxdrive-$COMPONENT"
            test_name="Component Tests: $COMPONENT"
            ;;
        *)
            echo "Unknown test type: $TEST_TYPE" >&2
            exit 1
            ;;
    esac

    # Capturar salida de tests
    local test_output="$LOG_DIR/test_output.txt"
    local test_json="$LOG_DIR/test_results.json"

    # Ejecutar tests con formato JSON cuando sea posible
    echo "Running: $test_name" >&2

    # Rust tests con --format json (nightly) o parsear salida normal
    if $test_cmd -- -Z unstable-options --format json > "$test_json" 2>&1; then
        : # Tests pasaron con formato JSON
    else
        # Fallback a formato normal
        $test_cmd > "$test_output" 2>&1 || true
    fi

    # Parsear resultados
    if [ -f "$test_json" ] && [ -s "$test_json" ]; then
        # Parsear formato JSON de cargo test
        local passed=$(grep -c '"event":"ok"' "$test_json" || echo "0")
        local failed=$(grep -c '"event":"failed"' "$test_json" || echo "0")
        local failures=$(grep '"event":"failed"' "$test_json" | jq -s '.' || echo "[]")

        RESULTS=$(jq -n \
            --arg name "$test_name" \
            --argjson passed "$passed" \
            --argjson failed "$failed" \
            --argjson failures "$failures" \
            '{
                "test_type": $name,
                "passed": $passed,
                "failed": $failed,
                "success": ($failed == 0),
                "failures": $failures
            }')
    else
        # Parsear formato de texto
        local output=$(cat "$test_output" 2>/dev/null || echo "")
        local passed=$(echo "$output" | grep -oP '\d+(?= passed)' | head -1 || echo "0")
        local failed=$(echo "$output" | grep -oP '\d+(?= failed)' | head -1 || echo "0")
        local failures=$(echo "$output" | grep -A 100 "failures:" | head -50 || echo "")

        RESULTS=$(jq -n \
            --arg name "$test_name" \
            --argjson passed "${passed:-0}" \
            --argjson failed "${failed:-0}" \
            --arg failures "$failures" \
            --arg output "$output" \
            '{
                "test_type": $name,
                "passed": $passed,
                "failed": $failed,
                "success": ($failed == 0),
                "failure_details": $failures,
                "raw_output": $output
            }')
    fi
}

# Capturar logs del daemon durante los tests si esta corriendo
capture_daemon_logs() {
    if systemctl --user is-active lnxdrive &>/dev/null; then
        journalctl --user -u lnxdrive --since "now" -f -o json > "$LOG_DIR/daemon_logs.json" &
        DAEMON_LOG_PID=$!
    fi
}

stop_log_capture() {
    if [ -n "$DAEMON_LOG_PID" ]; then
        kill "$DAEMON_LOG_PID" 2>/dev/null || true
        wait "$DAEMON_LOG_PID" 2>/dev/null || true
    fi
}

# Main
capture_daemon_logs
run_tests
stop_log_capture

# Agregar logs capturados al resultado
if [ -f "$LOG_DIR/daemon_logs.json" ]; then
    DAEMON_LOGS=$(cat "$LOG_DIR/daemon_logs.json" | jq -s '.' 2>/dev/null || echo "[]")
    RESULTS=$(echo "$RESULTS" | jq --argjson logs "$DAEMON_LOGS" '.daemon_logs_during_test = $logs')
fi

# Salida final
if [ "$OUTPUT_FORMAT" = "json" ]; then
    echo "$RESULTS" | jq '{
        "command": "test",
        "timestamp": (now | todate),
        "results": .
    }'
else
    echo ""
    echo "==========================================================="
    echo "  $(echo "$RESULTS" | jq -r '.test_type')"
    echo "==========================================================="
    echo ""
    echo "Pasaron: $(echo "$RESULTS" | jq '.passed')"
    echo "Fallaron: $(echo "$RESULTS" | jq '.failed')"
    echo ""

    if [ "$(echo "$RESULTS" | jq '.success')" = "false" ]; then
        echo "Detalles de fallos:"
        echo "$RESULTS" | jq -r '.failure_details // .failures' 2>/dev/null
    else
        echo "Todos los tests pasaron"
    fi
    echo "==========================================================="
fi

# Exit code basado en exito
[ "$(echo "$RESULTS" | jq '.success')" = "true" ] && exit 0 || exit 1
```

### 10.3 Interfaz para Agentes de IA

#### 10.3.1 Protocolo de Comunicacion

Los agentes de IA pueden interactuar con el sistema de pruebas usando comandos con salida JSON:

```bash
# El agente ejecuta:
make validate OUTPUT=json

# Recibe respuesta estructurada:
{
    "command": "validate",
    "success": false,
    "duration_seconds": 12.34,
    "timestamp": "2026-01-29T15:30:00Z",
    "checks": [
        {"name": "format", "passed": true, "message": "Code is properly formatted"},
        {"name": "lint", "passed": false, "message": "Clippy found 2 error(s)", "details": "..."},
        {"name": "compile", "passed": true, "message": "Code compiles successfully"},
        {"name": "unit_tests", "passed": true, "message": "156 tests passed"}
    ],
    "summary": {
        "total": 4,
        "passed": 3,
        "failed": 1
    }
}
```

#### 10.3.2 Flujo de Trabajo del Agente

```
+---------------------------------------------------------------------------+
|  FLUJO DE TRABAJO: AGENTE DE IA DESARROLLANDO CON LNXDRIVE               |
+---------------------------------------------------------------------------+
|                                                                           |
|  1. Agente recibe tarea: "Implementar feature X"                          |
|                          |                                                |
|                          v                                                |
|  2. Agente modifica codigo                                                |
|                          |                                                |
|                          v                                                |
|  3. Agente ejecuta: make validate OUTPUT=json                             |
|                          |                                                |
|              +-----------+-----------+                                    |
|              |                       |                                    |
|              v                       v                                    |
|         SUCCESS                   FAILURE                                 |
|              |                       |                                    |
|              |                       v                                    |
|              |            4. Agente analiza JSON:                         |
|              |               - Que check fallo?                           |
|              |               - Cual es el detalle del error?              |
|              |                       |                                    |
|              |                       v                                    |
|              |            5. Agente corrige codigo                        |
|              |                       |                                    |
|              |                       +----------+                         |
|              |                                  |                         |
|              v                                  v                         |
|  6. Agente ejecuta: make diagnose OUTPUT=json (si necesita mas info)      |
|                          |                                                |
|                          v                                                |
|  7. Agente reporta resultado al usuario                                   |
|                                                                           |
+---------------------------------------------------------------------------+
```

#### 10.3.3 Ejemplos de Uso por Agentes

```bash
# ==============================================================================
# COMANDOS QUE UN AGENTE DE IA PUEDE EJECUTAR DURANTE CODIFICACION
# ==============================================================================

# 1. Validacion rapida despues de cambiar codigo (< 30s)
make validate OUTPUT=json

# 2. Probar solo el componente modificado
make test-core OUTPUT=json      # Si modifico lnxdrive-core
make test-fuse OUTPUT=json      # Si modifico lnxdrive-fuse
make test-graph OUTPUT=json     # Si modifico lnxdrive-graph

# 3. Obtener diagnostico si algo no funciona
make diagnose OUTPUT=json

# 4. Obtener solo el estado rapido (sin logs historicos)
make diagnose-quick OUTPUT=json

# 5. Analizar logs recientes buscando errores especificos
make analyze-logs OUTPUT=json FILTER="error|panic|failed"

# 6. Ejecutar tests de integracion (mas lento, ~2-5 min)
make test-integration OUTPUT=json

# 7. Ver si el daemon esta respondiendo
make diagnose-quick OUTPUT=json | jq '.diagnostics.dbus_service_active'
```

### 10.4 Script de Analisis de Logs Interactivo

```bash
#!/bin/bash
# scripts/dev-analyze-logs.sh
# Analiza logs y produce resumen estructurado
# Uso: ./dev-analyze-logs.sh --output json [--filter "pattern"] [--since "1 hour ago"]

OUTPUT_FORMAT="${OUTPUT:-human}"
FILTER=""
SINCE="1 hour ago"

while [[ $# -gt 0 ]]; do
    case $1 in
        --output) OUTPUT_FORMAT="$2"; shift 2 ;;
        --filter) FILTER="$2"; shift 2 ;;
        --since) SINCE="$2"; shift 2 ;;
        *) FILTER="$1"; shift ;;  # Permitir filtro sin flag
    esac
done

analyze_logs() {
    local logs_json=$(journalctl --user -u lnxdrive --since "$SINCE" -o json 2>/dev/null | jq -s '.')

    # Aplicar filtro si existe
    if [ -n "$FILTER" ]; then
        logs_json=$(echo "$logs_json" | jq --arg f "$FILTER" '[.[] | select(.MESSAGE | test($f; "i"))]')
    fi

    # Analisis
    local total=$(echo "$logs_json" | jq 'length')
    local errors=$(echo "$logs_json" | jq '[.[] | select(.PRIORITY == "3")] | length')
    local warnings=$(echo "$logs_json" | jq '[.[] | select(.PRIORITY == "4")] | length')

    # Agrupar por tipo de mensaje (primeras palabras)
    local message_groups=$(echo "$logs_json" | jq '
        group_by(.MESSAGE | split(" ")[0:3] | join(" ")) |
        map({
            pattern: .[0].MESSAGE | split(" ")[0:3] | join(" "),
            count: length,
            sample: .[0].MESSAGE
        }) |
        sort_by(-.count) |
        .[0:10]
    ')

    # Trace IDs unicos (para correlacion)
    local trace_ids=$(echo "$logs_json" | jq '[.[] | .TRACE_ID // empty] | unique')

    jq -n \
        --argjson total "$total" \
        --argjson errors "$errors" \
        --argjson warnings "$warnings" \
        --argjson groups "$message_groups" \
        --argjson traces "$trace_ids" \
        --argjson logs "$logs_json" \
        --arg since "$SINCE" \
        --arg filter "$FILTER" \
        '{
            "analysis": {
                "period": $since,
                "filter": $filter,
                "total_entries": $total,
                "errors": $errors,
                "warnings": $warnings,
                "unique_traces": ($traces | length),
                "top_message_patterns": $groups
            },
            "trace_ids": $traces,
            "logs": $logs
        }'
}

ANALYSIS=$(analyze_logs)

if [ "$OUTPUT_FORMAT" = "json" ]; then
    echo "$ANALYSIS" | jq '{
        "command": "analyze-logs",
        "timestamp": (now | todate),
        "result": .
    }'
else
    echo "==========================================================="
    echo "         ANALISIS DE LOGS"
    echo "==========================================================="
    echo ""
    echo "Periodo: $(echo "$ANALYSIS" | jq -r '.analysis.period')"
    echo "Filtro: $(echo "$ANALYSIS" | jq -r '.analysis.filter // "ninguno"')"
    echo ""
    echo "Resumen:"
    echo "  Total: $(echo "$ANALYSIS" | jq '.analysis.total_entries')"
    echo "  Errores: $(echo "$ANALYSIS" | jq '.analysis.errors')"
    echo "  Warnings: $(echo "$ANALYSIS" | jq '.analysis.warnings')"
    echo "  Traces unicos: $(echo "$ANALYSIS" | jq '.analysis.unique_traces')"
    echo ""
    echo "Patrones mas frecuentes:"
    echo "$ANALYSIS" | jq -r '.analysis.top_message_patterns[] | "  [\(.count)] \(.pattern)..."'
    echo "==========================================================="
fi
```

### 10.5 Integracion con IDEs y Editores

#### 10.5.1 Tasks de VS Code para Agentes

```json
// .vscode/tasks.json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "LNXDrive: Validate (JSON)",
            "type": "shell",
            "command": "make validate OUTPUT=json",
            "group": "test",
            "presentation": {
                "echo": true,
                "reveal": "always",
                "panel": "shared"
            },
            "problemMatcher": []
        },
        {
            "label": "LNXDrive: Diagnose (JSON)",
            "type": "shell",
            "command": "make diagnose OUTPUT=json",
            "problemMatcher": []
        },
        {
            "label": "LNXDrive: Test Component",
            "type": "shell",
            "command": "make test-${input:component} OUTPUT=json",
            "problemMatcher": []
        }
    ],
    "inputs": [
        {
            "id": "component",
            "type": "pickString",
            "description": "Select component to test",
            "options": ["core", "fuse", "graph", "audit", "cli"]
        }
    ]
}
```

### 10.6 Resumen: Principios de Automatizacion

```
+---------------------------------------------------------------------------+
|  PRINCIPIOS DE DEPURACION AUTOMATIZADA                                    |
+---------------------------------------------------------------------------+
|                                                                           |
|  1. UN COMANDO = UN RESULTADO                                             |
|     - Cada operacion de depuracion es un solo comando                     |
|     - No requiere pasos manuales intermedios                              |
|     - Falla explicitamente con informacion util                           |
|                                                                           |
|  2. SALIDA ESTRUCTURADA SIEMPRE DISPONIBLE                                |
|     - Todos los comandos soportan OUTPUT=json                             |
|     - JSON incluye: success, details, logs, suggestions                   |
|     - Parseable por humanos Y por agentes de IA                           |
|                                                                           |
|  3. FEEDBACK RAPIDO                                                       |
|     - Validacion completa < 30 segundos                                   |
|     - Tests por componente < 1 minuto                                     |
|     - Diagnostico rapido < 5 segundos                                     |
|                                                                           |
|  4. LOGS SIEMPRE CAPTURADOS                                               |
|     - Los tests capturan logs del daemon automaticamente                  |
|     - Los diagnosticos incluyen logs recientes                            |
|     - Correlacion via trace_id                                            |
|                                                                           |
|  5. AISLAMIENTO POR DEFECTO                                               |
|     - Los comandos no modifican el sistema permanentemente                |
|     - Archivos temporales se limpian automaticamente                      |
|     - Facil de ejecutar repetidamente                                     |
|                                                                           |
+---------------------------------------------------------------------------+
```

---

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Pipeline CI/CD](06-ci-cd-pipeline.md) - Makefile y estructura de proyecto
- [Logging y Tracing](07-logging-tracing.md) - Sistema de logging unificado
- [Mocking de APIs Externas](05-mocking-apis.md) - Tests con mocks
