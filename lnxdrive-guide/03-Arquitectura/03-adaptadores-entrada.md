# Adaptadores de Entrada (UI Intercambiables)

> **Ubicación:** `03-Arquitectura/03-adaptadores-entrada.md`
> **Relacionado:** [Capas y Puertos](02-capas-y-puertos.md), [UI GNOME](../04-Componentes/02-ui-gnome.md)

---

## Resumen de Adaptadores

| Adaptador | Toolkit | Entorno Objetivo | Características |
|-----------|---------|------------------|-----------------|
| `gnome-adapter` | GTK4 + libadwaita | GNOME 43+ | Online Accounts, Nautilus extension, dark mode |
| `kde-adapter` | Qt6 + KDE Frameworks | KDE Plasma 5.27+ | Dolphin integration, KStatusNotifierItem |
| `xfce-adapter` | GTK3 | XFCE, MATE | Thunar actions, tray icon tradicional |
| `cosmic-adapter` | iced (Rust) | Cosmic (System76) | Nativo Rust, Wayland-first |
| `cli-adapter` | Terminal | Universal | JSON output, scriptable, pipes |
| `dbus-adapter` | DBus | Background service | IPC para cualquier cliente |

## Principio de Diseño

Las interfaces de usuario son **adaptadores**, no el sistema. El núcleo expone una API estable a través de DBus, y cada UI es un cliente de esa API.

```
┌──────────────────────────────────────────────────────────────────┐
│                        lnxdrive-daemon                             │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  DBus Service: org.enigmora.LNXDrive                       │    │
│  │  ─────────────────────────────────────────────────────── │    │
│  │  Interfaces:                                             │    │
│  │  • org.enigmora.LNXDrive.Sync                              │    │
│  │  • org.enigmora.LNXDrive.Auth                              │    │
│  │  • org.enigmora.LNXDrive.Conflicts                         │    │
│  │  • org.enigmora.LNXDrive.State                             │    │
│  │  • org.enigmora.LNXDrive.Metrics                           │    │
│  │                                                          │    │
│  │  Signals:                                                │    │
│  │  • SyncProgressChanged(path, progress)                   │    │
│  │  • ItemStateChanged(path, old_state, new_state)          │    │
│  │  • ConflictDetected(conflict_id, details)                │    │
│  │  • ErrorOccurred(error_code, explanation)                │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          │                   │                   │
          ▼                   ▼                   ▼
   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
   │ lnxdrive-gtk  │    │ lnxdrive-qt   │    │ lnxdrive-iced │
   │ (GNOME)     │    │ (KDE)       │    │ (Cosmic)    │
   └─────────────┘    └─────────────┘    └─────────────┘
```

## Detalle por Adaptador

### GNOME (GTK4 + libadwaita)

Componentes:
- **GNOME Online Accounts Provider**: Aparece en Settings → Online Accounts
- **Nautilus Extension**: Overlay icons, menú contextual, columnas personalizadas
- **Panel de Preferencias**: Integrado en gnome-control-center
- **Status Indicator**: GNOME Shell Extension para top bar

### KDE Plasma (Qt6)

Componentes:
- **Dolphin ServiceMenu**: Menú contextual via .desktop files
- **System Tray**: KStatusNotifierItem con badge de progreso
- **KCM Module**: Aparece en System Settings → Cloud & Accounts
- **Plasma Applet** (opcional): Widget para panel/escritorio

### Cosmic (iced)

Componentes:
- **Cosmic Files Integration**: Plugin nativo para cosmic-files
- **Applet de Panel**: Usa API de applets de cosmic-panel
- **Página de Settings**: Integrado en cosmic-settings

### XFCE/MATE (GTK3)

Componentes:
- **Thunar Custom Actions**: Configuración via GUI de Thunar
- **MATE/Caja Extension**: Similar a Nautilus
- **System Tray**: XEmbed/SNI tradicional
- **Aplicación Standalone**: GTK3 simple y ligera

### CLI Universal

```bash
# Operaciones básicas
lnxdrive sync                      # Sincronizar ahora
lnxdrive status                    # Estado actual
lnxdrive status --json             # Output estructurado para scripts

# Files on Demand
lnxdrive hydrate ~/OneDrive/file.zip    # Descargar archivo
lnxdrive dehydrate ~/OneDrive/old/      # Liberar espacio
lnxdrive pin ~/OneDrive/important/      # Mantener siempre offline

# Conflictos
lnxdrive conflicts                 # Listar conflictos
lnxdrive conflicts resolve abc123 --keep-local
lnxdrive conflicts preview abc123  # Ver diff antes de resolver

# Auditoría
lnxdrive explain ~/OneDrive/file.txt    # ¿Por qué este estado?
lnxdrive audit --since "2 hours ago"    # Historial de acciones
```

---

## Ver también

- [Capas y Puertos](02-capas-y-puertos.md) - Interfaces del core
- [UI GNOME](../04-Componentes/02-ui-gnome.md) - Detalle del adaptador GNOME
- [UI KDE Plasma](../04-Componentes/03-ui-kde-plasma.md) - Detalle del adaptador KDE
- [CLI](../04-Componentes/06-cli.md) - Detalle de la interfaz de línea de comandos
- [Comunicación DBus](../08-Distribucion/02-comunicacion-dbus.md) - API DBus completa
