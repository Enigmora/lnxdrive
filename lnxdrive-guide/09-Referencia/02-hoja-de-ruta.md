# Hoja de Ruta Propuesta

> **Ubicación:** `09-Referencia/02-hoja-de-ruta.md`
> **Relacionado:** [Resumen Ejecutivo](../01-Vision/01-resumen-ejecutivo.md), [Estructura de Repositorios](../08-Distribucion/01-estructura-repositorios.md)

---

## Visión General

La hoja de ruta de LNXDrive está organizada en 11 fases (0-10), desde la infraestructura de testing hasta la publicación de artefactos reutilizables. Cada fase construye sobre las anteriores y puede ejecutarse de manera incremental.

---

## Fase 0: Infraestructura de Testing

**Objetivo:** Establecer la base de desarrollo seguro y testing.

- [ ] Configuración de proyecto con estructura de tests (unit/integration/e2e)
- [ ] Mock server de Microsoft Graph (wiremock)
- [ ] Scripts de desarrollo aislado (`dev-fuse-isolated.sh`, `dev-gnome-nested.sh`)
- [ ] Containerfiles para testing (systemd, FUSE, desktop)
- [ ] Pipeline CI/CD básico (GitHub Actions)
- [ ] VM de testing con GNOME provisionada
- [ ] Makefile con comandos de desarrollo

**Entregables:**
- Entorno de desarrollo reproducible
- Scripts de automatización
- Documentación de setup

---

## Fase 1: Fundamentos (Core + CLI)

**Objetivo:** Implementar el núcleo de sincronización y la interfaz de línea de comandos.

- [ ] Arquitectura hexagonal del core en Rust
- [ ] Implementación de Microsoft Graph adapter
- [ ] Implementación de SQLite state repository
- [ ] Motor de sincronización básico (upload/download)
- [ ] Delta sync funcional
- [ ] Rate limiter adaptativo (proactivo + reactivo)
- [ ] CLI con comandos básicos
- [ ] Servicio systemd (con tests de integración)

**Entregables:**
- `lnxdrive-core` crate
- `lnxdrive-cli` binario
- `lnxdrive-daemon` servicio
- Sincronización básica funcional

---

## Fase 2: Files-on-Demand

**Objetivo:** Implementar el sistema de archivos virtual con placeholders.

- [ ] Wrapper FUSE moderno (`lnxdrive-fuse` como crate independiente)
- [ ] Placeholders y estados de archivo
- [ ] Hidratación on-demand
- [ ] Deshidratación automática
- [ ] Extended attributes para metadata

**Entregables:**
- `lnxdrive-fuse` crate
- Integración con file managers
- Optimización de espacio en disco

---

## Fase 3: Integración GNOME

**Objetivo:** Proveer experiencia nativa en GNOME.

- [ ] DBus service completo
- [ ] Extensión Nautilus (overlay icons + menú)
- [ ] Panel de preferencias GTK4
- [ ] GNOME Shell extension para status
- [ ] Integración con GNOME Online Accounts

**Entregables:**
- Extensión de Nautilus
- Extensión de GNOME Shell
- Panel de configuración
- Soporte para GNOME 45+

---

## Fase 4: Observabilidad

**Objetivo:** Implementar sistema de monitoreo y diagnóstico.

- [ ] Sistema de auditoría (`lnxdrive-audit` como crate independiente)
- [ ] Métricas Prometheus
- [ ] Logs estructurados JSON
- [ ] Comando `lnxdrive explain`
- [ ] Sistema de telemetría opt-in (`lnxdrive-telemetry`)
  - [ ] Agente como proceso separado
  - [ ] Backend OpenTelemetry en Google Cloud
  - [ ] Anonimización de datos sensibles
  - [ ] CLI: subcomando `report`

**Entregables:**
- `lnxdrive-audit` crate
- `lnxdrive-telemetry` binario
- Dashboard de métricas
- Herramientas de diagnóstico
- Documentación de troubleshooting

---

## Fase 5: Conflictos

**Objetivo:** Implementar detección y resolución de conflictos.

- [ ] Motor de detección de conflictos (`lnxdrive-conflict` como crate independiente)
- [ ] Sistema de reglas YAML
- [ ] UI de resolución visual
- [ ] Integración con diff tools (Meld)

**Entregables:**
- `lnxdrive-conflict` crate
- Interfaz de resolución de conflictos
- Configuración de políticas por usuario

---

## Fase 6: Multi-Cuenta

**Objetivo:** Soportar múltiples cuentas OneDrive simultáneamente.

- [ ] Modelo de namespaces (`{provider}:{alias}`)
- [ ] Configuración multi-cuenta en YAML
- [ ] CLI con operaciones por cuenta y globales
- [ ] DBus API para gestión de cuentas
- [ ] Polling escalonado y recursos compartidos

**Entregables:**
- Soporte para N cuentas
- Gestión unificada de cuentas
- Optimización de recursos

---

## Fase 7: Más Entornos Desktop

**Objetivo:** Expandir soporte a otros escritorios Linux.

- [ ] Adaptador KDE (Qt6 + Dolphin)
- [ ] Adaptador XFCE (Thunar actions)
- [ ] Adaptador Cosmic (iced)

**Entregables:**
- Integración con Dolphin (KDE)
- Integración con Thunar (XFCE)
- Soporte experimental para Cosmic

---

## Fase 8: Multi-Proveedor

**Objetivo:** Extender soporte a otros servicios de nube.

- [ ] Puerto `ICloudProvider` estabilizado
- [ ] Adaptador Google Drive
- [ ] Adaptador Dropbox
- [ ] Adaptador Nextcloud (WebDAV)
- [ ] Registry de proveedores dinámico

**Entregables:**
- API estable para proveedores
- Soporte para Google Drive
- Soporte para Dropbox
- Soporte para Nextcloud

---

## Fase 9: Avanzado OneDrive

**Objetivo:** Implementar características avanzadas de OneDrive.

- [ ] SharePoint/Business support
- [ ] Shared folders
- [ ] Historial de versiones
- [ ] Compartir enlaces desde file manager

**Entregables:**
- Soporte empresarial (OneDrive for Business)
- Funcionalidades de colaboración
- Integración con SharePoint

---

## Fase 10: Publicación de Artefactos

**Objetivo:** Publicar componentes reutilizables para la comunidad.

- [ ] Publicar `lnxdrive-fuse` en crates.io
- [ ] Publicar `lnxdrive-audit` en crates.io
- [ ] Publicar `lnxdrive-conflict` en crates.io
- [ ] Publicar `lnxdrive-ratelimit` en crates.io
- [ ] Documentación en docs.rs para cada crate

**Entregables:**
- Crates publicados en crates.io
- Documentación completa en docs.rs
- Ejemplos de uso para cada crate

---

## Diagrama de Dependencias

```
Fase 0: Infraestructura
    │
    ▼
Fase 1: Core + CLI ─────────────────────────┐
    │                                        │
    ├─────────────────┐                      │
    ▼                 ▼                      │
Fase 2: FUSE    Fase 4: Observabilidad      │
    │                 │                      │
    ▼                 │                      │
Fase 3: GNOME ◄───────┘                      │
    │                                        │
    ├─────────────────┬──────────────────────┤
    ▼                 ▼                      ▼
Fase 5: Conflictos   Fase 6: Multi-Cuenta   Fase 7: Desktop
    │                 │                      │
    └─────────────────┼──────────────────────┘
                      ▼
              Fase 8: Multi-Proveedor
                      │
                      ▼
              Fase 9: Avanzado OneDrive
                      │
                      ▼
              Fase 10: Publicación
```

---

## Estimación de Tiempos

| Fase | Duración Estimada | Dependencias |
|------|-------------------|--------------|
| Fase 0 | 2-3 semanas | Ninguna |
| Fase 1 | 6-8 semanas | Fase 0 |
| Fase 2 | 4-6 semanas | Fase 1 |
| Fase 3 | 4-6 semanas | Fase 2 |
| Fase 4 | 2-3 semanas | Fase 1 |
| Fase 5 | 3-4 semanas | Fase 1 |
| Fase 6 | 3-4 semanas | Fase 1 |
| Fase 7 | 4-6 semanas | Fase 3 |
| Fase 8 | 6-8 semanas | Fase 6 |
| Fase 9 | 4-6 semanas | Fase 8 |
| Fase 10 | 2-3 semanas | Todas |

**Total estimado:** 9-12 meses para versión completa

---

## Hitos Principales

### MVP (Minimum Viable Product)
- Fases 0-3 completadas
- Sincronización básica con Files-on-Demand
- Integración GNOME funcional

### Versión 1.0
- Fases 0-6 completadas
- Multi-cuenta
- Sistema de conflictos
- Observabilidad completa

### Versión 2.0
- Todas las fases completadas
- Multi-proveedor
- Soporte para todos los escritorios principales
- Crates publicados

---

## Ver también

- [Resumen Ejecutivo](../01-Vision/01-resumen-ejecutivo.md) - Visión del proyecto
- [Estructura de Repositorios](../08-Distribucion/01-estructura-repositorios.md) - Organización del código
- [Artefactos Reutilizables](../07-Extensibilidad/04-artefactos-reutilizables.md) - Crates independientes
- [Gobernanza del Proyecto](../08-Distribucion/03-gobernanza-proyecto.md) - Modelo de contribución
