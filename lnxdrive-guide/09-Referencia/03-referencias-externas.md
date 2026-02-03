# Referencias Externas

> **Ubicación:** `09-Referencia/03-referencias-externas.md`
> **Relacionado:** [Stack Tecnológico](../05-Implementacion/01-stack-tecnologico.md), [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md)

---

## Investigación Original

La propuesta arquitectónica de LNXDrive se basa en análisis de múltiples fuentes de IA y documentación técnica:

- **Ideas de Claude** — Análisis de viabilidad y throttling
- **Ideas de Copilot GPT** — Modelo multi-tenant y oportunidades
- **Ideas de Gemini** — Opciones nativas y detalles técnicos
- **Ideas de Qwen** — Stack tecnológico y oportunidades GNOME

---

## Tecnologías y Documentación

### Microsoft Graph API

- [Microsoft Graph API - OneDrive Overview](https://learn.microsoft.com/en-us/graph/onedrive-concept-overview)
  - Documentación oficial de la API para OneDrive
  - Endpoints para sincronización delta
  - Manejo de archivos y carpetas

- [Cloud Files API (Windows)](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)
  - Referencia de implementación de Files-on-Demand en Windows
  - Patrones de diseño para placeholders
  - Modelo de hidratación bajo demanda

### Arquitectura de Software

- [Arquitectura Hexagonal - AWS](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/hexagonal-architecture.html)
  - Guía de patrones de diseño en la nube
  - Principios de puertos y adaptadores
  - Ejemplos de implementación

- [Clean Architecture](https://dev.to/dyarleniber/hexagonal-architecture-and-clean-architecture-with-examples-48oi)
  - Comparación de arquitecturas limpias
  - Ejemplos prácticos
  - Beneficios de separación de concerns

### Entornos de Escritorio Linux

- [COSMIC Desktop - iced toolkit](https://www.phoronix.com/news/COSMIC-Desktop-Iced-Toolkit)
  - Nuevo escritorio de System76
  - Framework iced para UI en Rust
  - Integración nativa con Rust

### Filesystem y FUSE

- [FUSE Protocol - Kernel Documentation](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
  - Documentación oficial del kernel
  - Protocolo de comunicación
  - Consideraciones de rendimiento

### Rust Ecosystem

- [notify-rust](https://github.com/hoodie/notify-rust)
  - Notificaciones de escritorio desde Rust
  - Integración con D-Bus
  - Soporte para múltiples backends

- [fuser (Rust FUSE)](https://github.com/cberner/fuser)
  - Implementación FUSE moderna en Rust
  - Alto rendimiento
  - API ergonómica

- [graph-rs-sdk (Rust Graph client)](https://github.com/sreeise/graph-rs-sdk)
  - Cliente no oficial de Microsoft Graph
  - Alternativa para Rust
  - Cobertura de API

---

## Proyectos de Referencia

### Clientes OneDrive Existentes

- [abraunegg/onedrive](https://github.com/abraunegg/onedrive)
  - Cliente OneDrive en D lang
  - El más completo para Linux actualmente
  - Referencia para features y edge cases
  - **Limitación:** No tiene Files-on-Demand

- [rclone](https://rclone.org/onedrive/)
  - Herramienta de sincronización multi-nube
  - Soporte para OneDrive
  - Modelo de montaje VFS
  - **Limitación:** No es específico para desktop

### Clientes de Referencia para Integración Desktop

- [Nextcloud Desktop Client](https://github.com/nextcloud/desktop)
  - Referencia excelente para overlay icons
  - Integración con múltiples file managers
  - Arquitectura de sincronización bien documentada
  - Implementación de Virtual Files (Files-on-Demand)

- [Dropbox Linux Client](https://www.dropbox.com/install-linux)
  - Referencia de UX para sincronización
  - Integración con Nautilus
  - Iconos de estado en bandeja

---

## Benchmarks y Datos de Rendimiento

- [TechEmpower Framework Benchmarks](https://www.techempower.com/benchmarks/)
  - Comparaciones de rendimiento entre lenguajes
  - Datos de Rust vs C#
  - Métricas de latencia y throughput

- [The Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/)
  - Comparaciones sintéticas de lenguajes
  - Uso de memoria
  - Tiempo de ejecución

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
  - Guía de optimización en Rust
  - Profiling
  - Mejores prácticas

---

## Documentación Técnica de Referencia

### Memoria y Ownership

- [Rust Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
  - Sistema de ownership
  - Borrowing
  - Lifetimes

### Linux Desktop Integration

- [D-Bus Specification](https://dbus.freedesktop.org/doc/dbus-specification.html)
  - Protocolo de comunicación IPC
  - Tipos de datos
  - Patrones de uso

- [freedesktop.org File Manager Integration](https://www.freedesktop.org/wiki/)
  - Estándares de escritorio Linux
  - XDG specifications
  - Integración de aplicaciones

- [GNOME Developer Documentation](https://developer.gnome.org/)
  - Desarrollo de extensiones GNOME
  - GTK4 y libadwaita
  - GJS para GNOME Shell

- [KDE Developer Documentation](https://develop.kde.org/)
  - Desarrollo para Plasma
  - Qt6 y KDE Frameworks
  - Integración con Dolphin

---

## Estándares y Especificaciones

- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
  - Ubicación de configuración
  - Cache y datos de usuario
  - Runtime directories

- [Extended Attributes (xattr)](https://man7.org/linux/man-pages/man7/xattr.7.html)
  - Metadata adicional en archivos
  - Usado para estado de sincronización
  - Compatibilidad entre filesystems

- [systemd User Services](https://www.freedesktop.org/software/systemd/man/systemd.unit.html)
  - Servicios en espacio de usuario
  - Socket activation
  - Integración con journal

---

## Herramientas de Desarrollo

### Testing y CI/CD

- [GitHub Actions](https://docs.github.com/en/actions)
  - CI/CD para el proyecto
  - Workflows de testing
  - Publicación de artefactos

- [Podman](https://podman.io/)
  - Containers sin daemon
  - Soporte para systemd en containers
  - Alternativa a Docker para desarrollo

### Debugging y Profiling

- [Jaeger Tracing](https://www.jaegertracing.io/)
  - Distributed tracing
  - Visualización de traces
  - Integración con OpenTelemetry

- [Prometheus](https://prometheus.io/)
  - Métricas y monitoreo
  - Alertas
  - Grafana dashboards

---

## Referencias Históricas (.NET)

> **Nota:** Estas referencias corresponden a tecnologías .NET que fueron evaluadas durante
> la fase de diseño pero descartadas en favor de Rust. Se mantienen como contexto para
> entender las decisiones arquitectónicas. Ver [Justificación de Rust](../05-Implementacion/02-justificacion-rust.md).

### Frameworks UI

- [Avalonia UI](https://avaloniaui.net/)
  - Framework cross-platform .NET
  - Evaluado como alternativa para UI unificada
  - Descartado: integración desktop menos nativa

### FUSE en .NET

- [Tmds.Fuse](https://github.com/tmds/Tmds.Fuse)
  - Binding FUSE para .NET
  - Descartado: pausas de GC incompatibles con requisitos de latencia

### Rendimiento .NET

- [.NET Performance Blog](https://devblogs.microsoft.com/dotnet/category/performance/)
  - Mejoras de rendimiento en .NET 8/9
  - Native AOT, GC optimizations

- [.NET Garbage Collection](https://learn.microsoft.com/en-us/dotnet/standard/garbage-collection/)
  - Funcionamiento del GC (razón principal del descarte)
  - Generaciones y pausas

- [.NET Native AOT](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/)
  - Compilación ahead-of-time
  - Evaluado pero insuficiente para eliminar pausas de GC

---

## Ver también

- [Stack Tecnológico](../05-Implementacion/01-stack-tecnologico.md) - Tecnologías seleccionadas para LNXDrive
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Fundamentos arquitectónicos
- [Análisis de Rendimiento](01-analisis-rendimiento.md) - Benchmarks Rust vs .NET
- [Anexo B: Benchmarks completos](../Anexos/B-benchmarks-rust-vs-dotnet.md) - Análisis técnico detallado
