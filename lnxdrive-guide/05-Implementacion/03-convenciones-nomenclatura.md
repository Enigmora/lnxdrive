# Convenciones de Nomenclatura

> **Ubicacion:** `05-Implementacion/03-convenciones-nomenclatura.md`
> **Relacionado:** [Patrones Rust](04-patrones-rust.md), [Stack Tecnologico](01-stack-tecnologico.md)

---

## Estructura del Repositorio Mono-repo

```
┌─────────────────────────────────────────────────────────────────────┐
│  Repositorio mono-repo con workspaces:                              │
│                                                                     │
│  lnxdrive/                                                          │
│  ├── Cargo.toml              (workspace root)                       │
│  ├── crates/                                                        │
│  │   ├── lnxdrive-fuse/        → publicado como crate independiente │
│  │   ├── lnxdrive-audit/       → publicado como crate independiente │
│  │   ├── lnxdrive-ratelimit/   → publicado como crate independiente │
│  │   ├── lnxdrive-conflict/    → publicado como crate independiente │
│  │   ├── lnxdrive-state/       → publicado como crate independiente │
│  │   ├── lnxdrive-graph/       → interno (no publicado)             │
│  │   └── lnxdrive-core/        → interno (no publicado)             │
│  └── apps/                                                          │
│      ├── lnxdrive-daemon/      → binario principal                  │
│      ├── lnxdrive-cli/         → binario CLI                        │
│      └── lnxdrive-gnome/       → adaptador GNOME                    │
│                                                                     │
│  Versionado:                                                        │
│  • Crates publicos: semver independiente                            │
│  • Crates internos: version del proyecto                            │
│  • Apps: version unificada del release                              │
│                                                                     │
│  Publicacion:                                                       │
│  • crates.io para Rust                                              │
│  • Documentacion en docs.rs                                         │
│  • Ejemplos en cada crate                                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## 19.1 Convenciones de Nomenclatura (Rust API Guidelines)

### Reglas de Casing

| Construccion | Convencion | Ejemplos |
|--------------|------------|----------|
| Tipos, Traits | `UpperCamelCase` | `FileSync`, `SyncError`, `WatchManager` |
| Funciones, Metodos, Variables | `snake_case` | `sync_file()`, `buffer_size`, `last_modified` |
| Constantes, Statics | `SCREAMING_SNAKE_CASE` | `MAX_BUFFER_SIZE`, `DEFAULT_TIMEOUT` |
| Modulos | `snake_case` | `file_watcher`, `sync_engine` |
| Features de Cargo | `kebab-case` | `async-std`, `full-sync` |

### Reglas para Acronimos

```rust
// UpperCamelCase: acronimo como una palabra
pub struct HttpClient;     // ✓ Correcto
pub struct HTTPClient;     // ✗ Incorrecto

pub struct JsonParser;     // ✓ Correcto
pub struct JSONParser;     // ✗ Incorrecto

pub struct Uuid;           // ✓ Correcto
pub struct UUID;           // ✗ Incorrecto

// snake_case: acronimo en minusculas
fn parse_json() {}         // ✓ Correcto
fn parse_JSON() {}         // ✗ Incorrecto

let http_client = ...;     // ✓ Correcto
let HTTP_client = ...;     // ✗ Incorrecto
```

### Convenciones de Metodos

```rust
// Getters: SIN prefijo "get_" para acceso simple
impl FileEntry {
    // ✓ Correcto - acceso simple
    pub fn path(&self) -> &Path { &self.path }
    pub fn size(&self) -> u64 { self.size }
    pub fn is_directory(&self) -> bool { self.is_dir }

    // ✓ Correcto - usar "get" solo cuando hay ambiguedad
    pub fn get(&self, key: &str) -> Option<&Value> { ... }
}

// Conversiones: seguir convencion as_/to_/into_
impl SyncPath {
    // as_ : borrowed → borrowed (barato)
    pub fn as_path(&self) -> &Path { &self.0 }
    pub fn as_str(&self) -> &str { ... }

    // to_ : borrowed → owned (costoso, puede fallar)
    pub fn to_string(&self) -> String { ... }
    pub fn to_path_buf(&self) -> PathBuf { ... }

    // into_ : owned → owned (consume self)
    pub fn into_path_buf(self) -> PathBuf { self.0 }
    pub fn into_string(self) -> String { ... }
}

// Predicados: prefijo is_, has_, can_, should_
impl FileEntry {
    pub fn is_hidden(&self) -> bool { ... }
    pub fn has_conflicts(&self) -> bool { ... }
    pub fn can_sync(&self) -> bool { ... }
    pub fn should_upload(&self) -> bool { ... }
}
```

## Nombres de Crates del Proyecto

| Crate | Proposito | Publicacion |
|-------|-----------|-------------|
| `lnxdrive-core` | Logica de negocio compartida | Interno |
| `lnxdrive-fuse` | Filesystem virtual FUSE | crates.io |
| `lnxdrive-audit` | Sistema de auditoria | crates.io |
| `lnxdrive-ratelimit` | Rate limiting para APIs | crates.io |
| `lnxdrive-conflict` | Motor de deteccion de conflictos | crates.io |
| `lnxdrive-state` | Gestion de estado | crates.io |
| `lnxdrive-graph` | Cliente Microsoft Graph | Interno |
| `lnxdrive-daemon` | Binario del daemon | App |
| `lnxdrive-cli` | Binario CLI | App |
| `lnxdrive-gnome` | Adaptador GNOME | App |
| `lnxdrive-kde` | Adaptador KDE | App |
| `lnxdrive-cosmic` | Adaptador Cosmic | App |

## Estructura de Directorios del Crate Core

```
lnxdrive-core/
├── src/
│   ├── lib.rs
│   │
│   ├── domain/                 # Capa de dominio (sin dependencias externas)
│   │   ├── mod.rs
│   │   ├── models/             # Entidades y value objects
│   │   │   ├── mod.rs
│   │   │   ├── file_entry.rs
│   │   │   ├── sync_state.rs
│   │   │   └── conflict.rs
│   │   ├── errors.rs           # Errores de dominio
│   │   └── services/           # Logica de negocio pura
│   │       ├── mod.rs
│   │       ├── conflict_resolver.rs
│   │       └── delta_calculator.rs
│   │
│   ├── ports/                  # Interfaces (traits) - contratos
│   │   ├── mod.rs
│   │   ├── inbound/            # Ports de entrada (casos de uso)
│   │   │   ├── mod.rs
│   │   │   ├── sync_service.rs
│   │   │   └── file_service.rs
│   │   └── outbound/           # Ports de salida (dependencias)
│   │       ├── mod.rs
│   │       ├── file_system.rs
│   │       ├── cloud_storage.rs
│   │       ├── cache.rs
│   │       └── event_bus.rs
│   │
│   ├── adapters/               # Implementaciones concretas
│   │   ├── mod.rs
│   │   ├── inbound/            # Adaptadores de entrada
│   │   │   ├── mod.rs
│   │   │   ├── cli_handler.rs
│   │   │   └── dbus_handler.rs
│   │   └── outbound/           # Adaptadores de salida
│   │       ├── mod.rs
│   │       ├── local_fs.rs
│   │       ├── onedrive_api.rs
│   │       ├── sqlite_cache.rs
│   │       └── tokio_events.rs
│   │
│   └── application/            # Orquestacion y DI
│       ├── mod.rs
│       ├── sync_coordinator.rs
│       └── container.rs        # Dependency injection
```

---

## Ver tambien

- [Patrones Rust](04-patrones-rust.md) - Patrones de diseno idiomaticos
- [Stack Tecnologico](01-stack-tecnologico.md) - Vision general del stack
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Filosofia arquitectonica
