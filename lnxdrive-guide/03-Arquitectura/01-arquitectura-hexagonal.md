# Arquitectura Hexagonal

> **Ubicación:** `03-Arquitectura/01-arquitectura-hexagonal.md`
> **Relacionado:** [Capas y Puertos](02-capas-y-puertos.md), [Adaptadores de Entrada](03-adaptadores-entrada.md)

---

## Filosofía: Arquitectura Limpia + Puertos y Adaptadores

Adoptamos una combinación de [Arquitectura Limpia](https://dev.to/dyarleniber/hexagonal-architecture-and-clean-architecture-with-examples-48oi) (Robert C. Martin) y [Arquitectura Hexagonal](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/hexagonal-architecture.html) (Alistair Cockburn) para lograr:

- **Independencia del framework de UI** — El núcleo no conoce GTK, Qt ni iced
- **Independencia de la infraestructura** — El núcleo no conoce FUSE, DBus ni Microsoft Graph
- **Testabilidad total** — Cada capa puede probarse en aislamiento
- **Intercambiabilidad de componentes** — Cambiar de GTK a Qt no requiere reescribir lógica de negocio

## Diagrama de Arquitectura

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                        ADAPTADORES DE ENTRADA (DRIVING)                       │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      │
│  │   CLI   │ │  GTK4   │ │   Qt6   │ │  GTK3   │ │  iced   │ │  DBus   │      │
│  │ (TUI)   │ │ GNOME   │ │  KDE    │ │ XFCE/   │ │ Cosmic  │ │ Service │      │
│  │         │ │libadwait│ │ Plasma  │ │  MATE   │ │         │ │         │      │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘      │
│       │           │           │           │           │           │           │
│       └───────────┴───────────┴─────┬─────┴───────────┴───────────┘           │
│                                     │                                         │
│                          ┌──────────▼──────────┐                              │
│                          │   PUERTOS ENTRADA   │                              │
│                          │  (Interfaces/Traits)│                              │
│                          │ ─────────────────── │                              │
│                          │ • ISyncController   │                              │
│                          │ • IAuthController   │                              │
│                          │ • IConflictResolver │                              │
│                          │ • IStateObserver    │                              │
│                          └──────────┬──────────┘                              │
├─────────────────────────────────────┼─────────────────────────────────────────┤
│                                     │                                         │
│  ┌──────────────────────────────────▼────────────────────────────────────┐    │
│  │                                                                       │    │
│  │                     NÚCLEO DE DOMINIO (CORE)                          │    │
│  │                                                                       │    │
│  │  ┌─────────────┐  ┌───────────────┐  ┌─────────────┐  ┌────────────┐  │    │
│  │  │  Entidades  │  │ Casos de Uso  │  │  Políticas  │  │  Estado    │  │    │
│  │  │ ─────────── │  │ ───────────-─ │  │ ─────────── │  │ ────────── │  │    │
│  │  │ • SyncItem  │  │ • SyncFile    │  │ • Conflict  │  │ • SyncState│  │    │
│  │  │ • FileState │  │ • ResolveConf │  │ • Exclusion │  │ • ItemState│  │    │
│  │  │ • Conflict  │  │ • Authenticate│  │ • Priority  │  │ • AuditLog │  │    │
│  │  │ • AuditEntry│  │ • QueryDelta  │  │ • Retry     │  │ • Metrics  │  │    │
│  │  └─────────────┘  └───────────────┘  └─────────────┘  └────────────┘  │    │
│  │                                                                       │    │
│  │                    Motor de Sincronización                            │    │
│  │  ┌────────────────────────────────────────────────────────────────┐   │    │
│  │  │ • Delta Resolution • Conflict Detection • State Machine        │   │    │
│  │  │ • Hash Verification • Chunk Management • Retry Orchestration   │   │    │
│  │  └────────────────────────────────────────────────────────────────┘   │    │
│  │                                                                       │    │
│  └──────────────────────────────────┬────────────────────────────────────┘    │
│                                     │                                         │
│                          ┌──────────▼───────────┐                             │
│                          │   PUERTOS SALIDA     │                             │
│                          │  (Interfaces/Traits) │                             │
│                          │ ───────────────────  │                             │
│                          │ • ICloudProvider     │                             │
│                          │ • ILocalFileSystem   │                             │
│                          │ • IStateRepository   │                             │
│                          │ • INotificationSvc   │                             │
│                          │ • IMetricsExporter   │                             │
│                          └──────────┬───────────┘                             │
│                                     │                                         │
├─────────────────────────────────────┼─────────────────────────────────────────┤
│       ┌─────────────┬───────────┬───┴───┬───────────┬─────────────┐           │
│       │             │           │       │           │             │           │
│       ▼             ▼           ▼       ▼           ▼             ▼           │
│  ┌─────────┐  ┌──────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐ ┌─────────┐   │
│  │ MS Graph│  │  FUSE    │ │ SQLite  │ │  DBus   │ │Prometheus│ │ GVfs/   │   │
│  │   API   │  │Filesystem│ │  State  │ │ Notify  │ │ Metrics  │ │ GIO     │   │
│  └─────────┘  └──────────┘ └─────────┘ └─────────┘ └──────────┘ └─────────┘   │
│                    ADAPTADORES DE SALIDA (DRIVEN)                             │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Beneficios de Esta Arquitectura

### 1. Testabilidad

Cada capa puede probarse en aislamiento:
- **Core**: Tests unitarios con mocks de puertos
- **Adaptadores**: Tests de integración con servicios reales o simulados
- **E2E**: Tests completos en VM con entorno real

### 2. Intercambiabilidad

- Cambiar de SQLite a PostgreSQL: solo implementar nuevo adaptador de `IStateRepository`
- Añadir soporte para Google Drive: solo implementar nuevo adaptador de `ICloudProvider`
- Nueva UI para elementary OS: solo implementar nuevo adaptador de entrada

### 3. Mantenibilidad

- El core de negocio está aislado de cambios en APIs externas
- Los bugs de UI no afectan la lógica de sincronización
- Las actualizaciones de Microsoft Graph no requieren cambios en el core

---

## Ver también

- [Capas y Puertos](02-capas-y-puertos.md) - Descripción detallada de cada capa
- [Adaptadores de Entrada](03-adaptadores-entrada.md) - UIs intercambiables
- [Adaptadores de Salida](04-adaptadores-salida.md) - Infraestructura
- [Estrategia de Testing](../06-Testing/01-estrategia-testing.md) - Cómo probar cada capa
