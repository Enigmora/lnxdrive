# LNXDrive Guide - Extension Instructions

> **Copy the content below into your project's CLAUDE.md file.**
> This enables Claude Code to consult the LNXDrive design guide for planning and development tasks.

---

## Content to Add to CLAUDE.md


# LNXDrive Design Guide Reference

> This project is part of the **LNXDrive** ecosystem.
> A comprehensive design and development guide exists at: `../lnxdrive-guide/`

---

## When to Consult the Guide

**MANDATORY** - Consult the guide before:

| Situation | Action |
|-----------|--------|
| Planning new features or components | Load relevant architecture docs |
| Making design decisions | Check existing ADRs and principles |
| Implementing a component | Load component specification |
| Writing tests | Load testing strategy docs |
| Uncertain about patterns or conventions | Load implementation guidelines |

---

## Quick Document Reference

### Understanding the Project
```
../lnxdrive-guide/01-Vision/01-resumen-ejecutivo.md
../lnxdrive-guide/01-Vision/02-principios-rectores.md
```

### Architecture & Design
```
../lnxdrive-guide/03-Arquitectura/01-arquitectura-hexagonal.md
../lnxdrive-guide/03-Arquitectura/02-capas-y-puertos.md
```

### Component Specifications
```
../lnxdrive-guide/04-Componentes/01-files-on-demand-fuse.md    # FUSE
../lnxdrive-guide/04-Componentes/02-ui-gnome.md                # GNOME UI
../lnxdrive-guide/04-Componentes/03-ui-kde-plasma.md           # KDE UI
../lnxdrive-guide/04-Componentes/04-ui-gtk3.md                 # XFCE/MATE UI
../lnxdrive-guide/04-Componentes/05-ui-cosmic.md               # Cosmic UI
../lnxdrive-guide/04-Componentes/06-cli.md                     # CLI
../lnxdrive-guide/04-Componentes/07-motor-sincronizacion.md    # Sync engine
../lnxdrive-guide/04-Componentes/08-microsoft-graph.md         # OneDrive API
../lnxdrive-guide/04-Componentes/09-rate-limiting.md           # Throttling
../lnxdrive-guide/04-Componentes/10-file-watching-inotify.md   # File watching
../lnxdrive-guide/04-Componentes/11-conflictos.md              # Conflicts
../lnxdrive-guide/04-Componentes/12-auditoria.md               # Audit system
../lnxdrive-guide/04-Componentes/13-telemetria.md              # Telemetry
```

### Implementation Guidelines
```
../lnxdrive-guide/05-Implementacion/01-stack-tecnologico.md
../lnxdrive-guide/05-Implementacion/03-convenciones-nomenclatura.md
../lnxdrive-guide/05-Implementacion/04-patrones-rust.md
../lnxdrive-guide/05-Implementacion/05-configuracion-yaml.md
```

### Testing
```
../lnxdrive-guide/06-Testing/01-estrategia-testing.md
../lnxdrive-guide/06-Testing/03-testing-fuse.md
../lnxdrive-guide/06-Testing/05-mocking-apis.md
../lnxdrive-guide/06-Testing/06-ci-cd-pipeline.md
../lnxdrive-guide/06-Testing/09-testing-seguridad.md
```

### Multi-Provider & Extensibility
```
../lnxdrive-guide/07-Extensibilidad/01-arquitectura-multi-proveedor.md
../lnxdrive-guide/07-Extensibilidad/02-puerto-icloudprovider.md
../lnxdrive-guide/07-Extensibilidad/03-multi-cuenta-namespaces.md
```

### Risk Analysis
```
../lnxdrive-guide/.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md
```

---

## Navigation by Task Type

| Task | Documents to Load |
|------|-------------------|
| **Implement Core component** | `03-Arquitectura/02-capas-y-puertos.md`, `04-Componentes/07-motor-sincronizacion.md` |
| **Implement FUSE** | `04-Componentes/01-files-on-demand-fuse.md`, `06-Testing/03-testing-fuse.md` |
| **Implement GNOME UI** | `04-Componentes/02-ui-gnome.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implement KDE UI** | `04-Componentes/03-ui-kde-plasma.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implement CLI** | `04-Componentes/06-cli.md` |
| **Add new cloud provider** | `07-Extensibilidad/02-puerto-icloudprovider.md` |
| **Write tests** | `06-Testing/01-estrategia-testing.md` + component-specific doc |
| **Debug issues** | `06-Testing/07-logging-tracing.md`, `06-Testing/08-automatizacion-depuracion.md` |
| **Check roadmap** | `09-Referencia/02-hoja-de-ruta.md` |

---

## Loading Strategy

1. **Minimal load**: Only the specific task document
2. **Contextual load**: Task document + related architecture document
3. **Full load**: Only for major planning or refactoring tasks

> **Main index**: `../lnxdrive-guide/Guía-de-diseño-y-desarrollo.md`

---

*LNXDrive Guide v1.0 — [Enigmora](https://enigmora.com)*


---

## Notes for Integration

### Path Adjustment

The paths above assume the guide repository is a sibling directory:
```
lnxdrive/
├── lnxdrive-guide/     # This guide (design documentation)
├── lnxdrive/           # Core + CLI + Daemon (Rust)
├── lnxdrive-gnome/     # GNOME UI - GTK4 + libadwaita (Rust)
├── lnxdrive-gtk3/      # GTK3 UI - XFCE/Cinnamon/Mate (Rust)
├── lnxdrive-plasma/    # KDE Plasma UI - Qt6 + KDE Frameworks (C++/QML)
├── lnxdrive-cosmic/    # Cosmic UI - iced + libcosmic (Rust)
└── lnxdrive-packaging/ # Packaging (Flatpak, deb, rpm, AUR)
```

If your structure differs, adjust the `../lnxdrive-guide/` paths accordingly.

### Combining with DevTrail

If the implementation project also uses DevTrail, place this reference section **after** the DevTrail rules in CLAUDE.md. The DevTrail documentation rules apply to changes in the implementation repo; this section is for consulting the design guide.

---

*Generated for LNXDrive Guide — [Enigmora](https://enigmora.com)*
