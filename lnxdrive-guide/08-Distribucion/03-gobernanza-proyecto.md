# Gobernanza del Proyecto

> **Ubicación:** `08-Distribucion/03-gobernanza-proyecto.md`
> **Relacionado:** [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md), [Estructura de Repositorios](01-estructura-repositorios.md)

---

## Parte V: Gobernanza y Observabilidad

### 5.1 Politicas Declarativas (YAML)

LNXDrive utiliza archivos YAML para definir politicas de sincronizacion, permitiendo configuracion declarativa y versionable.

### 5.2 Auditoria y Explicabilidad

El sistema mantiene un registro completo de todas las operaciones de sincronizacion, permitiendo:
- Rastrear el origen de cada cambio
- Explicar por que se tomo una decision de sincronizacion
- Auditar el comportamiento del sistema

### 5.3 Metricas Prometheus

LNXDrive expone metricas en formato Prometheus para observabilidad:
- Archivos sincronizados
- Errores de sincronizacion
- Latencia de operaciones
- Estado de conexion
- Uso de recursos

---

## Gobernanza del Proyecto

### Roles

| Rol | Responsabilidad | Repositorios |
|-----|-----------------|--------------|
| **Core Maintainers** | Arquitectura, API D-Bus, releases | lnxdrive |
| **GNOME Maintainers** | UI GNOME, integracion Nautilus | lnxdrive-gnome |
| **GTK3 Maintainers** | UI para XFCE/Cinnamon/Mate | lnxdrive-gtk3 |
| **KDE Maintainers** | UI Plasma, Plasmoid | lnxdrive-plasma |
| **Cosmic Maintainers** | UI Cosmic | lnxdrive-cosmic |
| **Packaging Maintainers** | Flatpak, deb, rpm, AUR | lnxdrive-packaging |

### Proceso de Contribucion

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        FLUJO DE CONTRIBUCION                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Fork del repositorio                                                    │
│         │                                                                   │
│         ▼                                                                   │
│  2. Crear branch: feature/*, fix/*, docs/*                                 │
│         │                                                                   │
│         ▼                                                                   │
│  3. Desarrollar con commits convencionales                                  │
│     (feat:, fix:, docs:, refactor:, test:, chore:)                         │
│         │                                                                   │
│         ▼                                                                   │
│  4. Abrir Pull Request                                                      │
│         │                                                                   │
│         ▼                                                                   │
│  5. CI debe pasar                                                           │
│         │                                                                   │
│         ▼                                                                   │
│  6. Review por maintainer del repositorio                                   │
│         │                                                                   │
│         ▼                                                                   │
│  7. Squash & Merge a main                                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Commits Convencionales

El proyecto utiliza [Conventional Commits](https://www.conventionalcommits.org/) para mensajes de commit:

| Prefijo | Uso |
|---------|-----|
| `feat:` | Nueva funcionalidad |
| `fix:` | Correccion de bug |
| `docs:` | Cambios en documentacion |
| `refactor:` | Refactorizacion de codigo |
| `test:` | Adicion o modificacion de tests |
| `chore:` | Tareas de mantenimiento |

### Releases Coordinados

Para releases mayores que afectan la API D-Bus:

1. **lnxdrive** release primero (daemon + ipc)
2. UIs actualizan dependencia de `lnxdrive-ipc`
3. UIs hacen release
4. **lnxdrive-packaging** actualiza manifiestos
5. Release unificado anunciado

### Versionado Semantico

Todos los repositorios siguen [Semantic Versioning 2.0](https://semver.org/):

```
MAJOR.MINOR.PATCH

- MAJOR: Cambios incompatibles en API D-Bus o configuracion
- MINOR: Nuevas funcionalidades backwards-compatible
- PATCH: Bug fixes
```

---

## Gobernanza Documentaria con DevTrail

### Por Que DevTrail?

DevTrail es un sistema de gobernanza documentaria que permite:
- Rastrear decisiones de diseno
- Documentar cambios realizados por agentes de IA
- Mantener un historial de la evolucion del proyecto

### Estructura DevTrail en LNXDrive

```
.devtrail/
├── logs/           # Logs de sesiones de trabajo
├── decisions/      # Decisiones de diseno (ADR)
├── ethics/         # Consideraciones eticas
└── metrics/        # Metricas de gobernanza
```

### Tipos de Documentos

| Tipo | Proposito | Cuando Crear |
|------|-----------|--------------|
| **AILOG** | Documentar acciones realizadas por agentes | Despues de cada sesion de trabajo |
| **AIDEC** | Documentar decisiones de diseno | Cuando se toma una decision significativa |
| **ETH** | Documentar consideraciones eticas | Cuando hay implicaciones eticas |

### Niveles de Autonomia para Agentes

| Nivel | Descripcion | Requiere |
|-------|-------------|----------|
| **AUTONOMO** | No requiere revision previa | Nada |
| **REQUIERE AILOG** | Documentar, revisar post-hoc | AILOG |
| **REQUIERE AIDEC** | Documentar decision, revisar post-hoc | AIDEC |
| **REQUIERE APROBACION** | Crear ETH, esperar humano | ETH + Aprobacion |
| **PROHIBIDO** | Solo humanos | N/A |

### Acciones por Nivel

**AUTONOMO (No requiere revision previa):**
- Correccion de errores de sintaxis
- Formateo de codigo
- Actualizacion de dependencias menores

**REQUIERE AILOG (Documentar, revisar post-hoc):**
- Implementacion de features pequenas
- Refactorizacion local
- Adicion de tests

**REQUIERE AIDEC (Documentar decision, revisar post-hoc):**
- Cambios en arquitectura
- Adicion de nuevas dependencias
- Cambios en API publica

**REQUIERE APROBACION PREVIA:**
- Cambios en API D-Bus
- Modificacion de politicas de seguridad
- Cambios que afectan multiples repositorios

**PROHIBIDO (Solo humanos):**
- Releases de produccion
- Cambios en credenciales/secretos
- Eliminacion de datos de usuarios

---

## Metricas de Gobernanza

| Metrica | Descripcion |
|---------|-------------|
| Commits por agente vs humano | Proporcion de contribuciones |
| Decisiones revertidas | Calidad de decisiones de agentes |
| Tiempo de revision | Eficiencia del proceso de review |
| Cobertura de documentacion | Completitud de DevTrail |

---

## Ver tambien

- [01-estructura-repositorios.md](01-estructura-repositorios.md) - Estructura de repositorios
- [02-comunicacion-dbus.md](02-comunicacion-dbus.md) - Comunicacion D-Bus
- [04-internacionalizacion.md](04-internacionalizacion.md) - Estrategia de internacionalizacion
