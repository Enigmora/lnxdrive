# Adaptador Cosmic (iced)

> **Ubicación:** `04-Componentes/05-ui-cosmic.md`
> **Relacionado:** [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md)

---

### 4.4 Adaptador Cosmic (iced)

El [escritorio Cosmic de System76](https://www.phoronix.com/news/COSMIC-Desktop-Iced-Toolkit) usa [iced](https://github.com/iced-rs/iced), un toolkit Rust nativo. Esto presenta una oportunidad unica:

```
┌──────────────────────────────────────────────────────────────────┐
│  lnxdrive-cosmic (100% Rust)                                       │
│  ─────────────────────────────────────────────────────────────   │
│                                                                  │
│  Ventajas de integracion nativa:                                 │
│  • Mismo lenguaje que el core (si se implementa en Rust)         │
│  • Theming automatico con sistema Cosmic                         │
│  • Wayland-first sin hacks X11                                   │
│  • Memory safety compartida con el daemon                        │
│                                                                  │
│  Componentes:                                                    │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │  Cosmic Files Integration                                 │   │
│  │  • Plugin para cosmic-files (file manager de Cosmic)      │   │
│  │  • Overlay icons nativos                                  │   │
│  │  • Context menu via cosmic-comp                           │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │  Applet de Panel                                          │   │
│  │  • Usa API de applets de cosmic-panel                     │   │
│  │  • Estado en tiempo real                                  │   │
│  │  • Controles rapidos                                      │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │  Pagina de Settings                                       │   │
│  │  • Integrado en cosmic-settings                           │   │
│  │  • UI declarativa con iced                                │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Files-on-Demand FUSE](01-files-on-demand-fuse.md) - Sistema de archivos bajo demanda
- [Adaptador GNOME](02-ui-gnome.md) - Integracion con GNOME
- [Adaptador KDE Plasma](03-ui-kde-plasma.md) - Integracion con KDE
- [CLI Universal](06-cli.md) - Interfaz de linea de comandos
