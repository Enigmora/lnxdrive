# Principios Rectores

> **Ubicación:** `01-Vision/02-principios-rectores.md`
> **Relacionado:** [Resumen Ejecutivo](01-resumen-ejecutivo.md), [Propuesta de Valor](03-propuesta-de-valor.md)

---

## Principios Fundamentales

### 1. Solo nos detenemos ante lo imposible, no ante lo complejo

La complejidad técnica no es excusa para no implementar una característica valiosa. Si Windows puede tener Files-on-Demand con integración profunda, Linux puede tener una solución equivalente usando las herramientas disponibles (FUSE, extended attributes, GIO).

### 2. La sincronización es un sistema, no un truco

Un cliente de sincronización no es solo "subir y bajar archivos". Es un sistema completo que debe manejar:
- Estados de archivo y transiciones
- Detección y resolución de conflictos
- Recuperación de errores
- Comunicación con el usuario
- Persistencia de estado
- Observabilidad y auditoría

### 3. El usuario merece saber por qué algo falló

Cada error debe ser explicable. No basta con "Error de sincronización". El sistema debe poder responder:
- ¿Qué archivo falló?
- ¿Por qué falló?
- ¿Qué acciones se intentaron?
- ¿Qué puede hacer el usuario para resolverlo?
- ¿Cuándo se intentará de nuevo?

### 4. La UI es un detalle de implementación, no el corazón del sistema

El núcleo de sincronización debe ser completamente independiente de la interfaz de usuario. Esto permite:
- Adaptadores nativos para cada entorno de escritorio
- CLI completo para usuarios avanzados y scripts
- DBus API para integración con cualquier cliente
- Testing del core sin dependencias de UI

## Implicaciones Arquitectónicas

Estos principios llevan directamente a las decisiones arquitectónicas:

| Principio | Implicación |
|-----------|-------------|
| No detenerse ante lo complejo | FUSE + extended attributes para Files-on-Demand |
| Sincronización como sistema | Máquina de estados explícita para cada archivo |
| Explicabilidad | Sistema de auditoría con entradas estructuradas |
| UI como detalle | Arquitectura hexagonal con puertos y adaptadores |

---

## Ver también

- [Resumen Ejecutivo](01-resumen-ejecutivo.md) - Visión general del proyecto
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Cómo estos principios se implementan
- [Auditoría](../04-Componentes/12-auditoria.md) - Sistema de explicabilidad
