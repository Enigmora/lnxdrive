# Estructura de Repositorios y Gobernanza

> **Ubicación:** `08-Distribucion/01-estructura-repositorios.md`
> **Relacionado:** [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md), [Gobernanza](03-gobernanza-proyecto.md)

---

## Parte XVII: Estructura de Repositorios y Gobernanza

> **Objetivo:** Definir la organizacion de repositorios del proyecto LNXDrive, considerando la diversidad de entornos de escritorio Linux y la necesidad de mantener pipelines de CI/CD simples y manejables.

### 17.1 Filosofia: Pragmatismo sobre Purismo

LNXDrive adopta un enfoque **pragmatico** respecto a los lenguajes de programacion:

- **Rust** donde es natural y tiene ecosistema maduro
- **Lenguajes nativos del ecosistema** donde Rust presenta friccion significativa

Este pragmatismo se traduce en usar el toolkit y lenguaje que cada entorno de escritorio espera, maximizando la integracion nativa y facilitando contribuciones de desarrolladores especializados en cada ecosistema.

### 17.2 Analisis de Entornos de Escritorio Objetivo

| Escritorio | Toolkit Nativo | Lenguaje Ecosistema | Bindings Rust | Decision |
|------------|----------------|---------------------|---------------|----------|
| **GNOME** | GTK4 + libadwaita | Vala, Python, Rust | Excelentes (gtk4-rs, libadwaita-rs) | **Rust** |
| **KDE Plasma** | Qt6 + KDE Frameworks | C++, QML | Inmaduros (cxx-qt) | **C++ / QML** |
| **XFCE** | GTK3 | C, Python | Maduro (gtk3-rs) | **Rust** |
| **Cinnamon** | GTK3 + Cjs | Python, JavaScript | Maduro (gtk3-rs) | **Rust** |
| **Mate** | GTK3 | C, Python | Maduro (gtk3-rs) | **Rust** |
| **Cosmic** | iced + libcosmic | Rust | Nativo | **Rust** |

#### Agrupacion por Toolkit

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AGRUPACION POR TOOLKIT                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   GTK4 + libadwaita        GTK3                Qt6 + KDE         iced       │
│   ─────────────────        ────                ─────────         ────       │
│                                                                             │
│       ┌───────┐        ┌─────────┐            ┌─────────┐    ┌─────────┐   │
│       │ GNOME │        │  XFCE   │            │   KDE   │    │ Cosmic  │   │
│       └───────┘        │Cinnamon │            │ Plasma  │    └─────────┘   │
│                        │  Mate   │            └─────────┘                   │
│                        └─────────┘                                          │
│           │                 │                      │              │         │
│           ▼                 ▼                      ▼              ▼         │
│      ┌─────────┐       ┌─────────┐           ┌─────────┐    ┌─────────┐    │
│      │  Rust   │       │  Rust   │           │ C++/QML │    │  Rust   │    │
│      │ gtk4-rs │       │ gtk3-rs │           │   Qt6   │    │  iced   │    │
│      └─────────┘       └─────────┘           └─────────┘    └─────────┘    │
│                                                                             │
│   1 repositorio      1 repositorio          1 repositorio   1 repositorio  │
│                     (3 escritorios)                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 17.3 Estructura de Repositorios

```
github.com/enigmora/
│
├── lnxdrive/                       # CORE + CLI + DAEMON
│   │                               # ══════════════════
│   │                               # Lenguaje: Rust
│   │                               # El corazon del proyecto
│   │
│   ├── Cargo.toml                  # Workspace root
│   ├── crates/
│   │   ├── lnxdrive-core/          # Logica de negocio (hexagonal)
│   │   ├── lnxdrive-daemon/        # Servicio systemd
│   │   ├── lnxdrive-fuse/          # Adaptador FUSE
│   │   ├── lnxdrive-cli/           # Interfaz de linea de comandos
│   │   ├── lnxdrive-ipc/           # Libreria D-Bus para clientes
│   │   ├── lnxdrive-watch/         # File watching (Part XVI)
│   │   ├── lnxdrive-sync/          # Motor de sincronizacion
│   │   ├── lnxdrive-graph/         # Cliente Microsoft Graph
│   │   ├── lnxdrive-cache/         # Sistema de cache
│   │   ├── lnxdrive-conflict/      # Resolucion de conflictos
│   │   ├── lnxdrive-audit/         # Sistema de auditoria
│   │   └── lnxdrive-telemetry/     # Agente de telemetria (proceso separado)
│   ├── tests/                      # Tests de integracion
│   ├── docs/                       # Documentacion tecnica
│   └── .github/workflows/
│       ├── ci.yml                  # Build + test + clippy
│       ├── release.yml             # Publicacion de crates
│       └── security.yml            # Auditoria de dependencias
│
├── lnxdrive-gnome/                 # UI GNOME
│   │                               # ════════
│   │                               # Lenguaje: Rust
│   │                               # Toolkit: GTK4 + libadwaita
│   │
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── app.rs                  # GtkApplication
│   │   ├── window.rs               # AdwApplicationWindow
│   │   ├── widgets/                # Widgets personalizados
│   │   └── dbus_client.rs          # Cliente D-Bus (usa lnxdrive-ipc)
│   ├── data/
│   │   ├── icons/
│   │   ├── org.enigmora.LNXDrive.desktop
│   │   ├── org.enigmora.LNXDrive.metainfo.xml
│   │   └── org.enigmora.LNXDrive.gschema.xml
│   └── .github/workflows/
│       ├── ci.yml                  # Build + test
│       └── flatpak.yml             # Build Flatpak
│
├── lnxdrive-gtk3/                  # UI GTK3 (XFCE, Cinnamon, Mate)
│   │                               # ═══════════════════════════════
│   │                               # Lenguaje: Rust
│   │                               # Toolkit: GTK3
│   │
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── app.rs
│   │   ├── tray.rs                 # Indicador de bandeja (AppIndicator/StatusNotifier)
│   │   ├── preferences.rs          # Dialogo de preferencias
│   │   └── dbus_client.rs
│   ├── data/
│   │   ├── icons/
│   │   └── lnxdrive-gtk3.desktop
│   └── .github/workflows/
│       └── ci.yml
│
├── lnxdrive-plasma/                # UI KDE PLASMA
│   │                               # ══════════════
│   │                               # Lenguaje: C++ / QML
│   │                               # Toolkit: Qt6 + KDE Frameworks
│   │
│   ├── CMakeLists.txt
│   ├── src/
│   │   ├── main.cpp
│   │   ├── lnxdriveservice.cpp     # Backend C++
│   │   ├── lnxdriveservice.h
│   │   └── dbusclient.cpp          # Cliente D-Bus
│   ├── qml/
│   │   ├── main.qml
│   │   ├── StatusPage.qml
│   │   ├── SyncPage.qml
│   │   └── SettingsPage.qml
│   ├── plasmoid/                   # Widget de Plasma (opcional)
│   │   ├── metadata.json
│   │   └── contents/
│   │       └── ui/
│   │           └── main.qml
│   ├── icons/
│   └── .github/workflows/
│       └── ci.yml                  # CMake + Qt6
│
├── lnxdrive-cosmic/                # UI COSMIC
│   │                               # ═════════
│   │                               # Lenguaje: Rust
│   │                               # Toolkit: iced + libcosmic
│   │
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── app.rs                  # cosmic::Application
│   │   ├── pages/
│   │   │   ├── status.rs
│   │   │   ├── sync.rs
│   │   │   └── settings.rs
│   │   └── dbus_client.rs
│   ├── data/
│   │   └── icons/
│   └── .github/workflows/
│       └── ci.yml
│
└── lnxdrive-packaging/             # PACKAGING UNIFICADO
    │                               # ═══════════════════
    │                               # Scripts y configuraciones
    │
    ├── flatpak/
    │   ├── org.enigmora.LNXDrive.yml           # Manifiesto GNOME
    │   └── org.enigmora.LNXDrive.Gtk3.yml      # Manifiesto GTK3
    ├── deb/
    │   ├── lnxdrive-core/
    │   │   └── debian/
    │   ├── lnxdrive-gnome/
    │   │   └── debian/
    │   └── ...
    ├── rpm/
    │   ├── lnxdrive-core.spec
    │   ├── lnxdrive-gnome.spec
    │   └── ...
    ├── aur/
    │   ├── lnxdrive-core/
    │   │   └── PKGBUILD
    │   ├── lnxdrive-gnome/
    │   │   └── PKGBUILD
    │   └── ...
    ├── appimage/
    │   └── lnxdrive.AppImage.yml
    ├── scripts/
    │   ├── build-all.sh
    │   ├── test-packaging.sh
    │   └── release.sh
    └── .github/workflows/
        ├── build-packages.yml      # Build todos los paquetes
        ├── test-install.yml        # Test instalacion en containers
        └── release.yml             # Publicar releases
```

### 17.4 Resumen por Repositorio

| Repositorio | Lenguaje | Toolkit | Escritorios | LOC Estimado |
|-------------|----------|---------|-------------|--------------|
| `lnxdrive` | Rust | - | (backend) | ~30,000 |
| `lnxdrive-gnome` | Rust | gtk4-rs + libadwaita | GNOME | ~5,000 |
| `lnxdrive-gtk3` | Rust | gtk3-rs | XFCE, Cinnamon, Mate | ~4,000 |
| `lnxdrive-plasma` | C++/QML | Qt6 + KDE Frameworks | KDE Plasma | ~4,000 |
| `lnxdrive-cosmic` | Rust | iced + libcosmic | Cosmic | ~4,000 |
| `lnxdrive-packaging` | Shell/YAML | - | - | ~2,000 |

**Total: 6 repositorios, 5 en Rust, 1 en C++/QML**

### 17.7 Versionado y Compatibilidad

#### Esquema de Versionado

Todos los repositorios siguen [Semantic Versioning 2.0](https://semver.org/):

```
MAJOR.MINOR.PATCH

- MAJOR: Cambios incompatibles en API D-Bus o configuracion
- MINOR: Nuevas funcionalidades backwards-compatible
- PATCH: Bug fixes
```

#### Matriz de Compatibilidad

| lnxdrive (daemon) | lnxdrive-ipc | UIs compatibles |
|-------------------|--------------|-----------------|
| 1.0.x | 1.0.x | gnome 1.x, gtk3 1.x, plasma 1.x, cosmic 1.x |
| 1.1.x | 1.0.x - 1.1.x | gnome 1.x, gtk3 1.x, plasma 1.x, cosmic 1.x |
| 2.0.x | 2.0.x | gnome 2.x, gtk3 2.x, plasma 2.x, cosmic 2.x |

**Regla:** El daemon y `lnxdrive-ipc` comparten version MAJOR. Las UIs pueden tener versiones MAJOR independientes pero deben declarar compatibilidad con version del daemon.

#### Archivo de Compatibilidad

Cada UI incluye un archivo declarando compatibilidad:

```toml
# lnxdrive-gnome/compatibility.toml

[daemon]
min_version = "1.0.0"
max_version = "1.99.99"

[dbus_interface]
version = "1"
```

### 17.8 CI/CD por Repositorio

#### lnxdrive

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Format check
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Build
        run: cargo build --all-targets

      - name: Test
        run: cargo test --all-targets

  # Tests de integracion en container
  integration:
    runs-on: ubuntu-latest
    container:
      image: ghcr.io/enigmora/lnxdrive-test:latest
    steps:
      - uses: actions/checkout@v4
      - name: Integration tests
        run: cargo test --test '*' -- --test-threads=1

  # Auditoria de seguridad
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v1
```

#### lnxdrive-plasma (KDE)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    container:
      image: kdeorg/ci-suse-qt66:latest

    steps:
      - uses: actions/checkout@v4

      - name: Install dependencies
        run: |
          zypper install -y \
            extra-cmake-modules \
            kf6-ki18n-devel \
            kf6-kcoreaddons-devel \
            kf6-kdbusaddons-devel \
            kf6-kirigami-devel

      - name: Configure
        run: cmake -B build -DCMAKE_BUILD_TYPE=Release

      - name: Build
        run: cmake --build build --parallel

      - name: Test
        run: ctest --test-dir build --output-on-failure
```

### 17.9 Gobernanza del Proyecto

#### Roles

| Rol | Responsabilidad | Repositorios |
|-----|-----------------|--------------|
| **Core Maintainers** | Arquitectura, API D-Bus, releases | lnxdrive |
| **GNOME Maintainers** | UI GNOME, integracion Nautilus | lnxdrive-gnome |
| **GTK3 Maintainers** | UI para XFCE/Cinnamon/Mate | lnxdrive-gtk3 |
| **KDE Maintainers** | UI Plasma, Plasmoid | lnxdrive-plasma |
| **Cosmic Maintainers** | UI Cosmic | lnxdrive-cosmic |
| **Packaging Maintainers** | Flatpak, deb, rpm, AUR | lnxdrive-packaging |

#### Proceso de Contribucion

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        FLUJO DE CONTRIBUCION                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Fork del repositorio                                                    │
│         │                                                                   │
│         ▼                                                                   │
│  2. Crear branch: feature/*, fix/*, docs/*                                 │
│         │                                                                   │
│         ▼                                                                   │
│  3. Desarrollar con commits convencionales                                  │
│     (feat:, fix:, docs:, refactor:, test:, chore:)                         │
│         │                                                                   │
│         ▼                                                                   │
│  4. Abrir Pull Request                                                      │
│         │                                                                   │
│         ▼                                                                   │
│  5. CI debe pasar                                                           │
│         │                                                                   │
│         ▼                                                                   │
│  6. Review por maintainer del repositorio                                   │
│         │                                                                   │
│         ▼                                                                   │
│  7. Squash & Merge a main                                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Releases Coordinados

Para releases mayores que afectan la API D-Bus:

1. **lnxdrive** release primero (daemon + ipc)
2. UIs actualizan dependencia de `lnxdrive-ipc`
3. UIs hacen release
4. **lnxdrive-packaging** actualiza manifiestos
5. Release unificado anunciado

### 17.10 Dependencias entre Repositorios

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        GRAFO DE DEPENDENCIAS                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                           lnxdrive                                          │
│                    (publica lnxdrive-ipc)                                   │
│                              │                                              │
│            ┌─────────────────┼─────────────────┐                           │
│            │                 │                 │                           │
│            ▼                 ▼                 ▼                           │
│     lnxdrive-gnome    lnxdrive-gtk3    lnxdrive-cosmic                     │
│     (usa lnxdrive-ipc)                                                      │
│                                                                             │
│                                                                             │
│                        lnxdrive-plasma                                      │
│               (implementa D-Bus directamente)                               │
│                                                                             │
│                              │                                              │
│                              ▼                                              │
│                                                                             │
│                     lnxdrive-packaging                                      │
│                 (referencia todos los repos)                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Cargo.toml de UIs Rust

```toml
# lnxdrive-gnome/Cargo.toml

[package]
name = "lnxdrive-gnome"
version = "1.0.0"
edition = "2021"

[dependencies]
# Desde crates.io (cuando se publique)
lnxdrive-ipc = "1.0"

# O desde git durante desarrollo
# lnxdrive-ipc = { git = "https://github.com/enigmora/lnxdrive", branch = "main" }

gtk4 = "0.8"
libadwaita = "0.6"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

---

## Ver tambien

- [02-comunicacion-dbus.md](02-comunicacion-dbus.md) - Comunicacion D-Bus entre componentes
- [03-gobernanza-proyecto.md](03-gobernanza-proyecto.md) - Gobernanza y observabilidad del proyecto
- [04-internacionalizacion.md](04-internacionalizacion.md) - Estrategia de internacionalizacion (i18n)
