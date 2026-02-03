# Resumen Ejecutivo

> **Ubicación:** `01-Vision/01-resumen-ejecutivo.md`
> **Relacionado:** [Principios Rectores](02-principios-rectores.md), [Propuesta de Valor](03-propuesta-de-valor.md)

---

Este documento presenta una propuesta arquitectónica para construir un cliente de sincronización de almacenamiento en la nube para Linux que se distinga fundamentalmente de las soluciones existentes. El enfoque no es "otro cliente que sincroniza", sino un **sistema de sincronización explicable, gobernable y adaptable** que trata las interfaces de usuario como componentes intercambiables y soporta múltiples proveedores cloud.

## Visión del Proyecto

**LNXDrive** es un cliente de almacenamiento en la nube para Linux diseñado desde cero con principios de arquitectura limpia y hexagonal, que permite:

- **Sincronización explicable**: El usuario siempre sabe por qué algo falló o no se sincronizó
- **Gobernanza declarativa**: Configuración versionable en YAML, no flags crípticos
- **UI intercambiable**: Adaptadores nativos para GNOME, KDE, Cosmic, XFCE y CLI
- **Files-on-Demand real**: Implementación FUSE robusta con estados claros
- **Observabilidad completa**: Métricas Prometheus, logs JSON estructurados, audit trail

## Diferenciadores Clave

| Aspecto | Soluciones Actuales | LNXDrive |
|---------|---------------------|----------|
| **Explicabilidad** | "Error: sync failed" | "El archivo no se sincronizó porque el hash remoto cambió durante la subida. Se creó conflicto #xyz." |
| **Configuración** | Flags CLI, archivos .conf | YAML declarativo, versionable, auditable |
| **Observabilidad** | Logs de texto | Métricas Prometheus, logs JSON estructurados, audit trail |
| **Conflictos** | Renombrado automático silencioso | UI visual, reglas configurables, preview de resolución |
| **Files-on-Demand** | No existe o buggy | FUSE robusto con estados claros y overlay icons |
| **UI** | Una sola opción | Adaptadores intercambiables por DE |
| **Multi-cuenta** | Limitada o inexistente | Namespaces ilimitados (`{provider}:{alias}`) |
| **Multi-proveedor** | Específico a un servicio | Arquitectura extensible (OneDrive, GDrive, Dropbox, etc.) |

---

## Ver también

- [Principios Rectores](02-principios-rectores.md) - Filosofía del proyecto
- [Propuesta de Valor](03-propuesta-de-valor.md) - Diferenciadores detallados
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Diseño de alto nivel
- [Hoja de Ruta](../09-Referencia/02-hoja-de-ruta.md) - Plan de implementación
