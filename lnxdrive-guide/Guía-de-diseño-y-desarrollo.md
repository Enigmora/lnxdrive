# Guía de Diseño y Desarrollo de LNXDrive

> **Proyecto:** LNXDrive — Cliente de almacenamiento en la nube para Linux
> **Versión:** 1.0
> **Última actualización:** Enero 2026

---

## Introducción

Esta guía contiene la documentación técnica completa para el diseño y desarrollo de **LNXDrive**, un cliente de sincronización de almacenamiento en la nube para Linux con arquitectura hexagonal, Files-on-Demand real, y soporte multi-entorno de escritorio. Diseñado para soportar múltiples proveedores (OneDrive, Google Drive, Dropbox, entre otros).

### Cómo usar esta guía

Esta documentación está organizada por **etapas del ciclo de desarrollo de software**, optimizada para consulta modular sin agotar la ventana de contexto. Cada archivo es autocontenido y puede ser consultado de forma independiente.

**Para Agentes de IA:** Carga solo los archivos relevantes para la tarea actual. Usa la sección "Guía de Navegación para Agentes IA" al final de este documento.

---

## Mapa de la Guía

```
Guía_LNXDrive/
│
├── 01-Vision/                  ← Concepto y propuesta de valor
├── 02-Analisis/                ← Estado del arte y oportunidades
├── 03-Arquitectura/            ← Diseño de alto nivel
├── 04-Componentes/             ← Diseño detallado por módulo
├── 05-Implementacion/          ← Stack tecnológico y guías de código
├── 06-Testing/                 ← Estrategias de testing y depuración
├── 07-Extensibilidad/          ← Multi-proveedor y multi-cuenta
├── 08-Distribucion/            ← Repositorios, packaging, i18n
├── 09-Referencia/              ← Hoja de ruta y referencias
└── Anexos/                     ← Documentos técnicos completos
```

---

## Índice por Etapa de Desarrollo

### 1. Visión y Concepto
- [01-Vision/01-resumen-ejecutivo.md](01-Vision/01-resumen-ejecutivo.md) — Qué es LNXDrive y por qué
- [01-Vision/02-principios-rectores.md](01-Vision/02-principios-rectores.md) — Filosofía del proyecto
- [01-Vision/03-propuesta-de-valor.md](01-Vision/03-propuesta-de-valor.md) — Diferenciadores clave

### 2. Análisis y Oportunidades
- [02-Analisis/01-soluciones-existentes.md](02-Analisis/01-soluciones-existentes.md) — Competencia analizada
- [02-Analisis/02-brechas-y-oportunidades.md](02-Analisis/02-brechas-y-oportunidades.md) — Dónde diferenciarse

### 3. Arquitectura
- [03-Arquitectura/01-arquitectura-hexagonal.md](03-Arquitectura/01-arquitectura-hexagonal.md) — Diseño de alto nivel
- [03-Arquitectura/02-capas-y-puertos.md](03-Arquitectura/02-capas-y-puertos.md) — Core, puertos e interfaces
- [03-Arquitectura/03-adaptadores-entrada.md](03-Arquitectura/03-adaptadores-entrada.md) — UIs intercambiables
- [03-Arquitectura/04-adaptadores-salida.md](03-Arquitectura/04-adaptadores-salida.md) — Infraestructura

### 4. Componentes Detallados
- [04-Componentes/01-files-on-demand-fuse.md](04-Componentes/01-files-on-demand-fuse.md) — Implementación FUSE
- [04-Componentes/02-ui-gnome.md](04-Componentes/02-ui-gnome.md) — Adaptador GNOME
- [04-Componentes/03-ui-kde-plasma.md](04-Componentes/03-ui-kde-plasma.md) — Adaptador KDE
- [04-Componentes/04-ui-gtk3.md](04-Componentes/04-ui-gtk3.md) — Adaptador XFCE/MATE
- [04-Componentes/05-ui-cosmic.md](04-Componentes/05-ui-cosmic.md) — Adaptador Cosmic
- [04-Componentes/06-cli.md](04-Componentes/06-cli.md) — Interfaz de línea de comandos
- [04-Componentes/07-motor-sincronizacion.md](04-Componentes/07-motor-sincronizacion.md) — Motor de sync
- [04-Componentes/08-microsoft-graph.md](04-Componentes/08-microsoft-graph.md) — Integración API
- [04-Componentes/09-rate-limiting.md](04-Componentes/09-rate-limiting.md) — Throttling adaptativo
- [04-Componentes/10-file-watching-inotify.md](04-Componentes/10-file-watching-inotify.md) — Observación de archivos
- [04-Componentes/11-conflictos.md](04-Componentes/11-conflictos.md) — Resolución de conflictos
- [04-Componentes/12-auditoria.md](04-Componentes/12-auditoria.md) — Sistema de auditoría

### 5. Implementación
- [05-Implementacion/01-stack-tecnologico.md](05-Implementacion/01-stack-tecnologico.md) — Decisiones de stack
- [05-Implementacion/02-justificacion-rust.md](05-Implementacion/02-justificacion-rust.md) — Por qué Rust
- [05-Implementacion/03-convenciones-nomenclatura.md](05-Implementacion/03-convenciones-nomenclatura.md) — Estándares de código
- [05-Implementacion/04-patrones-rust.md](05-Implementacion/04-patrones-rust.md) — Patrones de diseño
- [05-Implementacion/05-configuracion-yaml.md](05-Implementacion/05-configuracion-yaml.md) — Configuración declarativa

### 6. Testing y Depuración
- [06-Testing/01-estrategia-testing.md](06-Testing/01-estrategia-testing.md) — Estrategia general
- [06-Testing/02-testing-systemd.md](06-Testing/02-testing-systemd.md) — Testing de servicios
- [06-Testing/03-testing-fuse.md](06-Testing/03-testing-fuse.md) — Testing de FUSE
- [06-Testing/04-testing-desktop.md](06-Testing/04-testing-desktop.md) — Testing de extensiones
- [06-Testing/05-mocking-apis.md](06-Testing/05-mocking-apis.md) — Mocking de APIs
- [06-Testing/06-ci-cd-pipeline.md](06-Testing/06-ci-cd-pipeline.md) — Pipeline CI/CD
- [06-Testing/07-logging-tracing.md](06-Testing/07-logging-tracing.md) — Logging y tracing
- [06-Testing/08-automatizacion-depuracion.md](06-Testing/08-automatizacion-depuracion.md) — Automatización

### 7. Extensibilidad
- [07-Extensibilidad/01-arquitectura-multi-proveedor.md](07-Extensibilidad/01-arquitectura-multi-proveedor.md) — Multi-cloud
- [07-Extensibilidad/02-puerto-icloudprovider.md](07-Extensibilidad/02-puerto-icloudprovider.md) — Interface de proveedores
- [07-Extensibilidad/03-multi-cuenta-namespaces.md](07-Extensibilidad/03-multi-cuenta-namespaces.md) — Multi-cuenta
- [07-Extensibilidad/04-artefactos-reutilizables.md](07-Extensibilidad/04-artefactos-reutilizables.md) — Crates publicables

### 8. Distribución
- [08-Distribucion/01-estructura-repositorios.md](08-Distribucion/01-estructura-repositorios.md) — Organización de repos
- [08-Distribucion/02-comunicacion-dbus.md](08-Distribucion/02-comunicacion-dbus.md) — API DBus
- [08-Distribucion/03-gobernanza-proyecto.md](08-Distribucion/03-gobernanza-proyecto.md) — Gobernanza
- [08-Distribucion/04-internacionalizacion.md](08-Distribucion/04-internacionalizacion.md) — i18n

### 9. Referencia
- [09-Referencia/01-analisis-rendimiento.md](09-Referencia/01-analisis-rendimiento.md) — Benchmarks resumen
- [09-Referencia/02-hoja-de-ruta.md](09-Referencia/02-hoja-de-ruta.md) — Plan de fases
- [09-Referencia/03-referencias-externas.md](09-Referencia/03-referencias-externas.md) — Links y recursos

### Anexos (Documentos Completos)
- [Anexos/A-metodologias-testing-completo.md](Anexos/A-metodologias-testing-completo.md) — Testing completo
- [Anexos/B-benchmarks-rust-vs-dotnet.md](Anexos/B-benchmarks-rust-vs-dotnet.md) — Análisis rendimiento

---

## Índice por Componente Técnico

### Core de Sincronización
| Tema | Archivo |
|------|---------|
| Motor de estado | [04-Componentes/07-motor-sincronizacion.md](04-Componentes/07-motor-sincronizacion.md) |
| Delta sync | [04-Componentes/08-microsoft-graph.md](04-Componentes/08-microsoft-graph.md) |
| Rate limiting | [04-Componentes/09-rate-limiting.md](04-Componentes/09-rate-limiting.md) |
| File watching | [04-Componentes/10-file-watching-inotify.md](04-Componentes/10-file-watching-inotify.md) |
| Conflictos | [04-Componentes/11-conflictos.md](04-Componentes/11-conflictos.md) |
| Auditoría | [04-Componentes/12-auditoria.md](04-Componentes/12-auditoria.md) |

### Files-on-Demand
| Tema | Archivo |
|------|---------|
| FUSE implementation | [04-Componentes/01-files-on-demand-fuse.md](04-Componentes/01-files-on-demand-fuse.md) |
| Estados de archivo | [04-Componentes/01-files-on-demand-fuse.md](04-Componentes/01-files-on-demand-fuse.md) |

### Interfaces de Usuario
| Entorno | Archivo |
|---------|---------|
| GNOME | [04-Componentes/02-ui-gnome.md](04-Componentes/02-ui-gnome.md) |
| KDE Plasma | [04-Componentes/03-ui-kde-plasma.md](04-Componentes/03-ui-kde-plasma.md) |
| XFCE/MATE | [04-Componentes/04-ui-gtk3.md](04-Componentes/04-ui-gtk3.md) |
| Cosmic | [04-Componentes/05-ui-cosmic.md](04-Componentes/05-ui-cosmic.md) |
| CLI | [04-Componentes/06-cli.md](04-Componentes/06-cli.md) |

### Extensibilidad
| Tema | Archivo |
|------|---------|
| Multi-proveedor | [07-Extensibilidad/01-arquitectura-multi-proveedor.md](07-Extensibilidad/01-arquitectura-multi-proveedor.md) |
| Multi-cuenta | [07-Extensibilidad/03-multi-cuenta-namespaces.md](07-Extensibilidad/03-multi-cuenta-namespaces.md) |
| Crates reutilizables | [07-Extensibilidad/04-artefactos-reutilizables.md](07-Extensibilidad/04-artefactos-reutilizables.md) |

---

## Índice Alfabético

| Archivo | Descripción |
|---------|-------------|
| `01-Vision/01-resumen-ejecutivo.md` | Visión general del proyecto |
| `01-Vision/02-principios-rectores.md` | Filosofía de diseño |
| `01-Vision/03-propuesta-de-valor.md` | Diferenciadores vs competencia |
| `02-Analisis/01-soluciones-existentes.md` | Análisis de clientes existentes |
| `02-Analisis/02-brechas-y-oportunidades.md` | Oportunidades de mercado |
| `03-Arquitectura/01-arquitectura-hexagonal.md` | Patrón arquitectónico |
| `03-Arquitectura/02-capas-y-puertos.md` | Entidades, casos de uso, puertos |
| `03-Arquitectura/03-adaptadores-entrada.md` | Adaptadores de UI |
| `03-Arquitectura/04-adaptadores-salida.md` | Adaptadores de infraestructura |
| `04-Componentes/01-files-on-demand-fuse.md` | Implementación FUSE |
| `04-Componentes/02-ui-gnome.md` | Integración GNOME |
| `04-Componentes/03-ui-kde-plasma.md` | Integración KDE |
| `04-Componentes/04-ui-gtk3.md` | Integración XFCE/MATE |
| `04-Componentes/05-ui-cosmic.md` | Integración Cosmic |
| `04-Componentes/06-cli.md` | Comandos CLI |
| `04-Componentes/07-motor-sincronizacion.md` | Lógica de sincronización |
| `04-Componentes/08-microsoft-graph.md` | API de OneDrive |
| `04-Componentes/09-rate-limiting.md` | Control de throttling |
| `04-Componentes/10-file-watching-inotify.md` | Observación de cambios |
| `04-Componentes/11-conflictos.md` | Resolución de conflictos |
| `04-Componentes/12-auditoria.md` | Logging y auditoría |
| `05-Implementacion/01-stack-tecnologico.md` | Tecnologías seleccionadas |
| `05-Implementacion/02-justificacion-rust.md` | Por qué Rust |
| `05-Implementacion/03-convenciones-nomenclatura.md` | Estándares de código |
| `05-Implementacion/04-patrones-rust.md` | Patrones de diseño |
| `05-Implementacion/05-configuracion-yaml.md` | Formato de configuración |
| `06-Testing/01-estrategia-testing.md` | Pirámide de testing |
| `06-Testing/02-testing-systemd.md` | Testing de servicios |
| `06-Testing/03-testing-fuse.md` | Testing de filesystem |
| `06-Testing/04-testing-desktop.md` | Testing de extensiones |
| `06-Testing/05-mocking-apis.md` | Mocks y stubs |
| `06-Testing/06-ci-cd-pipeline.md` | Automatización CI/CD |
| `06-Testing/07-logging-tracing.md` | Observabilidad |
| `06-Testing/08-automatizacion-depuracion.md` | Debugging automatizado |
| `07-Extensibilidad/01-arquitectura-multi-proveedor.md` | Soporte multi-cloud |
| `07-Extensibilidad/02-puerto-icloudprovider.md` | Interfaz de proveedores |
| `07-Extensibilidad/03-multi-cuenta-namespaces.md` | Múltiples cuentas |
| `07-Extensibilidad/04-artefactos-reutilizables.md` | Librerías publicables |
| `08-Distribucion/01-estructura-repositorios.md` | Organización del código |
| `08-Distribucion/02-comunicacion-dbus.md` | Protocolo IPC |
| `08-Distribucion/03-gobernanza-proyecto.md` | Gestión del proyecto |
| `08-Distribucion/04-internacionalizacion.md` | Traducciones |
| `09-Referencia/01-analisis-rendimiento.md` | Benchmarks |
| `09-Referencia/02-hoja-de-ruta.md` | Plan de desarrollo |
| `09-Referencia/03-referencias-externas.md` | Documentación externa |
| `Anexos/A-metodologias-testing-completo.md` | Guía completa de testing |
| `Anexos/B-benchmarks-rust-vs-dotnet.md` | Análisis completo de rendimiento |

---

## Guía de Navegación para Agentes IA

### Por tipo de tarea

| Tarea | Archivos a cargar |
|-------|-------------------|
| **Entender el proyecto** | `01-Vision/01-resumen-ejecutivo.md`, `01-Vision/02-principios-rectores.md` |
| **Implementar componente Core** | `03-Arquitectura/02-capas-y-puertos.md`, `04-Componentes/07-motor-sincronizacion.md` |
| **Implementar FUSE** | `04-Componentes/01-files-on-demand-fuse.md`, `06-Testing/03-testing-fuse.md` |
| **Implementar UI GNOME** | `04-Componentes/02-ui-gnome.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implementar UI KDE** | `04-Componentes/03-ui-kde-plasma.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implementar CLI** | `04-Componentes/06-cli.md` |
| **Añadir nuevo proveedor** | `07-Extensibilidad/02-puerto-icloudprovider.md` |
| **Configurar multi-cuenta** | `07-Extensibilidad/03-multi-cuenta-namespaces.md` |
| **Escribir tests** | `06-Testing/01-estrategia-testing.md` + archivo específico del componente |
| **Configurar CI/CD** | `06-Testing/06-ci-cd-pipeline.md` |
| **Depurar problema** | `06-Testing/07-logging-tracing.md`, `06-Testing/08-automatizacion-depuracion.md` |
| **Decisiones de rendimiento** | `09-Referencia/01-analisis-rendimiento.md` o `Anexos/B-benchmarks-rust-vs-dotnet.md` |
| **Ver roadmap** | `09-Referencia/02-hoja-de-ruta.md` |
| **Revisar riesgos del componente** | Sección "⚠️ Riesgos y Mitigaciones" en cada archivo `04-Componentes/*.md`. Matriz completa: `.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md` |
| **Tests de seguridad** | `06-Testing/09-testing-seguridad.md` |
| **Telemetría y reportes** | `04-Componentes/13-telemetria.md`, `06-Testing/07-logging-tracing.md` |

### Recomendaciones de carga

1. **Carga mínima**: Solo el archivo específico de la tarea
2. **Carga contextual**: Archivo de tarea + archivo de arquitectura relacionado
3. **Carga completa**: Solo para tareas de planificación o refactoring mayor

### Ejemplo de uso

```
Tarea: "Implementar hidratación de archivos en FUSE"

Cargar:
1. 04-Componentes/01-files-on-demand-fuse.md (implementación)
2. 03-Arquitectura/02-capas-y-puertos.md (puertos a implementar)
3. 06-Testing/03-testing-fuse.md (cómo probar)
```

---

*Guía generada el 30 de enero de 2026*
*Proyecto Enigmora — LNXDrive*
