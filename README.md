# LNXDrive Core

Motor de sincronización, daemon y CLI para LNXDrive - cliente OneDrive nativo para Linux.

## Descripción

Este repositorio contiene el backend completo de LNXDrive, implementado en Rust siguiendo una arquitectura hexagonal. Incluye:

- **lnxdrive-core**: Lógica de negocio y dominio
- **lnxdrive-daemon**: Servicio systemd para sincronización en segundo plano
- **lnxdrive-cli**: Interfaz de línea de comandos
- **lnxdrive-ipc**: Librería D-Bus compartida para clientes UI
- **lnxdrive-fuse**: Implementación FUSE para Files-on-Demand
- **lnxdrive-sync**: Motor de sincronización delta
- **lnxdrive-graph**: Cliente Microsoft Graph API
- **lnxdrive-cache**: Sistema de caché local
- **lnxdrive-conflict**: Detección y resolución de conflictos
- **lnxdrive-audit**: Sistema de auditoría y logging estructurado
- **lnxdrive-telemetry**: Agente de telemetría (opt-in)

## Arquitectura

```
┌─────────────────────────────────────────┐
│     ADAPTADORES DE ENTRADA              │
│     CLI, D-Bus (UIs externas)           │
└────────────────┬────────────────────────┘
                 │
         ┌───────▼────────┐
         │ Puertos Entrada │
         └───────┬────────┘
                 │
┌────────────────▼─────────────────────────┐
│         NÚCLEO DE DOMINIO                │
│  Entidades, Casos de Uso, Estados        │
└────────────────┬─────────────────────────┘
                 │
         ┌───────▼────────┐
         │ Puertos Salida  │
         └───────┬────────┘
                 │
┌────────────────▼─────────────────────────┐
│     ADAPTADORES DE SALIDA                │
│  MS Graph, FUSE, SQLite, Prometheus      │
└─────────────────────────────────────────┘
```

## Requisitos

- Rust 1.75+
- libfuse3-dev
- libdbus-1-dev
- SQLite 3.35+

## Compilación

```bash
cargo build --release
```

## Instalación del daemon

```bash
cargo install --path crates/lnxdrive-daemon
systemctl --user enable --now lnxdrive
```

## Uso del CLI

```bash
lnxdrive status          # Ver estado de sincronización
lnxdrive sync            # Forzar sincronización
lnxdrive conflicts       # Listar conflictos
lnxdrive explain <path>  # Explicar estado de un archivo
```

## Configuración

La configuración se almacena en `~/.config/lnxdrive/config.yaml`.

## Licencia

GPL-3.0-or-later
