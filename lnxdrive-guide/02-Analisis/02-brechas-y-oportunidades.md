# Brechas Identificadas y Oportunidades

> **Ubicación:** `02-Analisis/02-brechas-y-oportunidades.md`
> **Relacionado:** [Soluciones Existentes](01-soluciones-existentes.md), [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md)

---

## Brechas Identificadas

De las cuatro fuentes de investigación consultadas (Claude, Copilot, Gemini, Qwen), emergen estas **oportunidades de diferenciación**:

### 1. Files-on-Demand real en Linux

**Brecha:** Windows tiene integración profunda con cfapi; Linux no tiene equivalente elegante.

**Oportunidad:** Implementar Files-on-Demand usando FUSE + extended attributes + GIO, con estados claros y overlay icons en los administradores de archivos.

### 2. Gobernanza del estado

**Brecha:** Ningún cliente explica *por qué* algo no se sincronizó.

**Oportunidad:** Sistema de auditoría con entradas estructuradas que permita al usuario entender exactamente qué pasó con cada archivo.

### 3. Políticas declarativas

**Brecha:** Configuración imperativa (flags) vs. declarativa (YAML versionable).

**Oportunidad:** Archivo de configuración YAML que se pueda versionar con Git, con reglas por patrón glob para conflictos, exclusiones, y prioridades.

### 4. Observabilidad

**Brecha:** Logs crípticos vs. métricas estructuradas y auditables.

**Oportunidad:** Métricas Prometheus exportables, logs JSON estructurados con trace IDs, integración con herramientas de monitoreo estándar.

### 5. Resolución de conflictos visual

**Brecha:** Todos los clientes renombran automáticamente sin feedback.

**Oportunidad:** UI de resolución de conflictos con preview, comparación lado a lado, integración con herramientas diff (Meld), y reglas configurables por tipo de archivo.

### 6. Integración desktop moderna

**Brecha:** Extensiones para administradores de archivos son mínimas o inexistentes.

**Oportunidad:** Extensiones nativas para Nautilus, Dolphin, Thunar, y Cosmic Files con:
- Overlay icons para estados
- Menú contextual con acciones
- Columnas personalizadas con información de sync

### 7. UI intercambiable

**Brecha:** Todos los clientes están atados a un toolkit específico.

**Oportunidad:** Arquitectura hexagonal donde el núcleo expone DBus API, y las UIs son adaptadores intercambiables:
- GTK4 + libadwaita para GNOME
- Qt6 para KDE
- iced para Cosmic
- GTK3 para XFCE/MATE
- CLI para scripts y usuarios avanzados

## Matriz de Oportunidades

| Oportunidad | Esfuerzo | Impacto | Prioridad |
|-------------|----------|---------|-----------|
| Files-on-Demand FUSE | Alto | Crítico | P0 |
| Sistema de auditoría | Medio | Alto | P1 |
| Configuración YAML | Bajo | Alto | P1 |
| Métricas Prometheus | Bajo | Medio | P2 |
| UI de conflictos | Medio | Alto | P1 |
| Extensiones desktop | Alto | Alto | P1 |
| Arquitectura hexagonal | Alto | Crítico | P0 |

---

## Ver también

- [Soluciones Existentes](01-soluciones-existentes.md) - Análisis de competencia
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Diseño propuesto
- [Files-on-Demand FUSE](../04-Componentes/01-files-on-demand-fuse.md) - Implementación de on-demand
- [Auditoría](../04-Componentes/12-auditoria.md) - Sistema de explicabilidad
