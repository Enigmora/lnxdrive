# Stack Tecnológico Propuesto

> **Ubicacion:** `05-Implementacion/01-stack-tecnologico.md`
> **Relacionado:** [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md), [Justificacion Rust](02-justificacion-rust.md)

---

## 7.1 Decision Principal: Lenguaje del Core

Despues de analizar las opciones, se seleccionó **Rust** como lenguaje del core:

### Stack Seleccionado: Core en Rust

```
┌──────────────────────────────────────────────────────────────────┐
│  STACK RUST                                                      │
│  ─────────────────────────────────────────────────────────────── │
│                                                                  │
│  Core:                                                           │
│  • Rust (memory safety, performance, async nativo)               │
│  • tokio (runtime async)                                         │
│  • serde (serialización)                                         │
│  • sqlx (SQLite async)                                           │
│                                                                  │
│  FUSE:                                                           │
│  • fuser (bindings libfuse3 modernos)                            │
│                                                                  │
│  IPC:                                                            │
│  • zbus (DBus async nativo Rust)                                 │
│                                                                  │
│  UI GNOME:                                                       │
│  • gtk4-rs + libadwaita-rs                                       │
│                                                                  │
│  UI KDE:                                                         │
│  • cxx-qt (bindings Qt6 desde Rust)                              │
│                                                                  │
│  UI Cosmic:                                                      │
│  • iced (nativo, mismo lenguaje!)                                │
│                                                                  │
│  HTTP:                                                           │
│  • reqwest (cliente HTTP async)                                  │
│  • oauth2-rs (flujos OAuth2)                                     │
│                                                                  │
│  Ventajas:                                                       │
│  ✓ Un solo lenguaje para daemon + UI Cosmic                      │
│  ✓ Memory safety sin GC                                          │
│  ✓ Excelente soporte async                                       │
│  ✓ Creciente adopcion en Linux desktop                           │
│                                                                  │
│  Desventajas:                                                    │
│  ✗ Curva de aprendizaje empinada                                 │
│  ✗ Bindings GTK/Qt menos maduros que nativos                     │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Opcion B: Core en .NET (Evaluada y Descartada)

> **Nota histórica:** Esta opción fue evaluada durante la fase de diseño pero se descartó
> en favor de Rust. Se documenta aquí para contexto de las decisiones arquitectónicas.

**Razones del descarte:**

1. **Rendimiento FUSE incompatible** — El Garbage Collector de .NET introduce pausas
   impredecibles (hasta 50-100ms en GC Gen2) que violan el requisito de latencia <1ms
   para operaciones `getattr` en el filesystem virtual.

2. **Overhead de memoria** — El runtime .NET consume ~30-50MB base vs ~2-5MB de un
   binario Rust, crítico para un daemon que corre 24/7.

3. **Ecosistema FUSE inmaduro** — Las opciones disponibles (Tmds.Fuse, Mono.Fuse) están
   desactualizadas o tienen limitaciones significativas vs `fuser` en Rust.

4. **Consistencia con Cosmic** — El desktop Cosmic (System76) está escrito en Rust/iced,
   permitiendo compartir código entre daemon y UI.

**Ver análisis completo:** [Justificación de Rust](02-justificacion-rust.md) |
[Benchmarks Rust vs .NET](../Anexos/B-benchmarks-rust-vs-dotnet.md)

## 7.2 Recomendacion: Rust Core + UI Nativas por DE

```
┌─────────────────────────────────────────────────────────────────┐
│  ARQUITECTURA: RUST CORE + ADAPTADORES NATIVOS                  │
│  ─────────────────────────────────────────────────────────────  │
│                                                                 │
│  lnxdrive-daemon (Rust):                                        │
│  • Core de sincronizacion                                       │
│  • Motor FUSE                                                   │
│  • Servicio DBus                                                │
│  • Alto rendimiento, bajo consumo                               │
│                                                                 │
│  lnxdrive-gnome (Python + GTK4):                                │
│  • Maxima integracion GNOME                                     │
│  • Usa PyGObject (maduro, bien documentado)                     │
│  • Extension Nautilus en Python                                 │
│                                                                 │
│  lnxdrive-kde (C++ + Qt6):                                      │
│  • Maxima integracion KDE                                       │
│  • KDE Frameworks nativo                                        │
│                                                                 │
│  lnxdrive-cosmic (Rust + iced):                                 │
│  • Comparte codigo con daemon                                   │
│  • 100% Rust, integracion perfecta                              │
│                                                                 │
│  lnxdrive-cli (Rust):                                           │
│  • Comparte codigo con daemon                                   │
│  • Binario standalone                                           │
│                                                                 │
│  Comunicacion: DBus (org.enigmora.LNXDrive)                     │
│  • Protocolo estandar freedesktop                               │
│  • Cualquier lenguaje puede implementar cliente                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Justificacion de Rust](02-justificacion-rust.md) - Analisis de rendimiento detallado
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Filosofia arquitectonica
- [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md) - UI intercambiables
- [Patrones Rust](04-patrones-rust.md) - Patrones de diseno en Rust
