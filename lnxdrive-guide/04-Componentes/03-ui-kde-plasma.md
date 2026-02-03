# Adaptador KDE Plasma (Qt6)

> **Ubicación:** `04-Componentes/03-ui-kde-plasma.md`
> **Relacionado:** [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md)

---

### 4.3 Adaptador KDE Plasma (Qt6)

```
┌───────────────────────────────────────────────────────────────────┐
│  lnxdrive-kde                                                       │
│  ───────────────────────────────────────────────────────────────  │
│                                                                   │
│  Componentes:                                                     │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  Dolphin ServiceMenu                                      │    │
│  │  • Menu contextual via .desktop files                     │    │
│  │  • KIO slave para operaciones de archivos                 │    │
│  │  • Emblems para estados via KFileItemModel                │    │
│  └───────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  System Tray (KStatusNotifierItem)                        │    │
│  │  • Icono con badge de progreso                            │    │
│  │  • Menu completo con todas las acciones                   │    │
│  │  • Tooltip con estado actual                              │    │
│  └───────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  KCM Module (System Settings)                             │    │
│  │  • Aparece en System Settings → Cloud & Accounts          │    │
│  │  • QML para UI moderna                                    │    │
│  │  • Kirigami para mobile-friendly                          │    │
│  └───────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  Plasma Applet (opcional)                                 │    │
│  │  • Widget para panel/escritorio                           │    │
│  │  • Vista de sincronizacion en tiempo real                 │    │
│  └───────────────────────────────────────────────────────────┘    │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Files-on-Demand FUSE](01-files-on-demand-fuse.md) - Sistema de archivos bajo demanda
- [Adaptador GNOME](02-ui-gnome.md) - Integracion con GNOME
- [Adaptador Cosmic](05-ui-cosmic.md) - Integracion con Cosmic
- [CLI Universal](06-cli.md) - Interfaz de linea de comandos
