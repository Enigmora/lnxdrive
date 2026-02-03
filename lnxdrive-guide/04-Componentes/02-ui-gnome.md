# Adaptador GNOME (GTK4 + libadwaita)

> **Ubicación:** `04-Componentes/02-ui-gnome.md`
> **Relacionado:** [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md)

---

### 4.2 Adaptador GNOME (GTK4 + libadwaita)

```
┌──────────────────────────────────────────────────────────────────┐
│  lnxdrive-gnome                                                    │
│  ──────────────────────────────────────────────────────────────  │
│                                                                  │
│  Componentes:                                                    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │  GNOME Online Accounts Provider                         │     │
│  │  • Aparece en Settings → Online Accounts                │     │
│  │  • OAuth2 flow integrado con WebKitGTK                  │     │
│  │  • Single sign-on con cuenta Microsoft existente        │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  Nautilus Extension (Python/C)                           │    │
│  │  • Overlay icons para estado de archivo                  │    │
│  │  • Menu contextual: "Sync Now", "Make Offline", "Share"  │    │
│  │  • Column provider: Estado, Ultima Sync                  │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  Panel de Preferencias (libadwaita)                      │    │
│  │  • Integrado en gnome-control-center                     │    │
│  │  • Selective sync con tree view                          │    │
│  │  • Exclusion patterns visual                             │    │
│  │  • Conflict resolution settings                          │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  Status Indicator (GNOME Shell Extension)                │    │
│  │  • Icono en top bar con menu desplegable                 │    │
│  │  • Progress de sincronizacion actual                     │    │
│  │  • Acceso rapido a conflictos pendientes                 │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Files-on-Demand FUSE](01-files-on-demand-fuse.md) - Sistema de archivos bajo demanda
- [Adaptador KDE Plasma](03-ui-kde-plasma.md) - Integracion con KDE
- [Adaptador GTK3](04-ui-gtk3.md) - Integracion con XFCE/MATE
- [CLI Universal](06-cli.md) - Interfaz de linea de comandos
