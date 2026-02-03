# Resolucion de Conflictos Visual

> **Ubicacion:** `04-Componentes/11-conflictos.md`
> **Relacionado:** [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]], [[04-Componentes/12-auditoria|Auditoria]]

---

## Parte VI: Resolucion de Conflictos Visual

### 6.1 Flujo de Resolucion

```
┌───────────────────────────────────────────────────────────────────┐
│  DETECCION DE CONFLICTO                                           │
│  ───────────────────────────────────────────────────────────────  │
│                                                                   │
│  Condicion: local_hash != remote_hash && ambos modificados        │
│             despues de ultima sincronizacion exitosa              │
│                                                                   │
└───────────────────────────────┬───────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│  EVALUACION DE POLITICA                                          │
│  ─────────────────────────────────────────────────────────────   │
│                                                                  │
│  1. Buscar regla especifica en config.yaml                       │
│  2. Si no hay regla -> usar default_strategy                     │
│  3. Si strategy = "prompt" -> notificar al usuario               │
│                                                                  │
└───────────────────────────────┬──────────────────────────────────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                 │
              ▼                 ▼                 ▼
       ┌───────────┐     ┌───────────┐     ┌───────────┐
       │keep-local │     │keep-remote│     │ keep-both │
       └─────┬─────┘     └─────┬─────┘     └─────┬─────┘
             │                 │                 │
             ▼                 ▼                 ▼
       ┌───────────┐     ┌───────────┐     ┌───────────┐
       │  Subir    │     │ Descargar │     │ Renombrar │
       │  local    │     │  remoto   │     │ + ambos   │
       └───────────┘     └───────────┘     └───────────┘
```

### 6.2 UI de Resolucion (GTK4 Example)

```
┌─────────────────────────────────────────────────────────────────┐
│  [!] Conflicto Detectado                                    [X] │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Archivo: documento.docx                                        │
│  Ruta: ~/OneDrive/Trabajo/documento.docx                        │
│                                                                 │
│  ┌─────────────────────────┬─────────────────────────────────┐  │
│  │     VERSION LOCAL       │       VERSION REMOTA            │  │
│  ├─────────────────────────┼─────────────────────────────────┤  │
│  │  Modificado: Hoy 15:30  │  Modificado: Hoy 15:45          │  │
│  │  Tamano: 245 KB         │  Tamano: 248 KB                 │  │
│  │  Por: (este dispositivo)│  Por: PC-Trabajo                │  │
│  │                         │                                 │  │
│  │  [Vista previa]         │  [Vista previa]                 │  │
│  └─────────────────────────┴─────────────────────────────────┘  │
│                                                                 │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  [Comparar diferencias (abre Meld/diff)]                   │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌─ Resolucion ──────────────────────────────────────────────┐  │
│  │                                                           │  │
│  │  ( ) Mantener version local (sobrescribe remoto)          │  │
│  │  ( ) Mantener version remota (sobrescribe local)          │  │
│  │  (*) Mantener ambas (renombra local como _conflicto)      │  │
│  │                                                           │  │
│  │  [ ] Recordar esta decision para archivos .docx           │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │    Cancelar     │  │   Resolver      │  │  Resolver Todos │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 Configuracion de Politicas de Conflictos

```yaml
# ~/.config/lnxdrive/config.yaml
conflicts:
  # Estrategia por defecto
  default_strategy: prompt   # prompt | keep-local | keep-remote | keep-both

  # Reglas especificas por patron
  rules:
    - pattern: "**/*.docx"
      strategy: keep-both    # Documentos: siempre conservar ambos

    - pattern: "**/config/**"
      strategy: keep-remote  # Configs: preferir version del servidor

    - pattern: "**/src/**"
      strategy: prompt       # Codigo: siempre preguntar
```

### 6.4 Estrategias de Resolucion

| Estrategia | Descripcion | Uso recomendado |
|------------|-------------|-----------------|
| `prompt` | Pregunta al usuario | Archivos importantes, codigo fuente |
| `keep-local` | Sobrescribe remoto con local | Desarrollo local prioritario |
| `keep-remote` | Sobrescribe local con remoto | Configuraciones compartidas |
| `keep-both` | Renombra local y conserva ambos | Documentos, archivos criticos |

### 6.5 Integracion con Herramientas de Diff

LNXDrive puede integrarse con herramientas externas de comparacion:

- **Meld** - Herramienta visual de diff para GNOME
- **KDiff3** - Herramienta de merge para KDE
- **diff** - Herramienta de linea de comandos
- **vimdiff** - Diff integrado en Vim

---

## ⚠️ Riesgos y Mitigaciones

### C5: Conflict Resolution Race Condition

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P2 (Media) |
| **Componentes** | ConflictResolver, IStateRepository, MS Graph |
| **Simulación** | SIM-L2-004 |

**Descripción:**
Durante la resolución de un conflicto, la versión remota puede cambiar nuevamente. Si el sistema aplica la resolución sin verificar, puede sobrescribir cambios más recientes.

**Escenarios de Fallo:**
1. Usuario elige "mantener local", pero mientras tanto otro dispositivo actualiza el archivo
2. Resolución de múltiples conflictos en lote mientras hay sync activo
3. Timeout de red durante el apply de resolución

**Mitigación Propuesta:**
```rust
impl ConflictResolver {
    pub async fn apply_resolution(
        &self,
        conflict: &Conflict,
        strategy: ResolutionStrategy,
    ) -> Result<ResolutionResult> {
        // 1. Obtener versión actual del servidor
        let current_remote = self.graph_client
            .get_metadata(&conflict.item.remote_id)
            .await?;
        
        // 2. Verificar que no cambió desde la detección
        if current_remote.etag != conflict.remote_version.etag {
            return Err(ConflictError::VersionChanged {
                expected: conflict.remote_version.etag.clone(),
                actual: current_remote.etag,
                message: "El archivo remoto cambió durante la resolución".to_string(),
            });
        }
        
        // 3. Aplicar con If-Match header (optimistic locking)
        match strategy {
            ResolutionStrategy::KeepLocal => {
                self.graph_client
                    .upload_with_etag(
                        &conflict.item,
                        &conflict.remote_version.etag, // If-Match
                    )
                    .await?;
            }
            // ... otras estrategias
        }
        
        Ok(ResolutionResult::Applied)
    }
}
```

**Tests Requeridos:**
- `test_resolution_fails_on_version_change`
- `test_etag_mismatch_triggers_refresh`
- `test_batch_resolution_atomic`

---

### B6: Conflict File Naming Collision

| Atributo | Valor |
|----------|-------|
| **Prioridad** | P3 (Baja) |
| **Componentes** | ConflictNamer, ILocalFileSystem |
| **Simulación** | SIM-L4-005 |

**Descripción:**
Cuando se genera un nombre para archivo de conflicto (ej: `file_conflicto_2026-01-31.docx`), puede colisionar con un archivo existente o con otro conflicto generado simultáneamente.

**Escenarios de Fallo:**
1. Múltiples conflictos del mismo archivo en el mismo día
2. Usuario ya tiene archivo con nombre `_conflicto_`
3. Race entre generación de nombres

**Mitigación Propuesta:**
```rust
pub struct ConflictNamer {
    counter: AtomicU32,
}

impl ConflictNamer {
    pub fn generate_unique_name(&self, original: &Path) -> PathBuf {
        let stem = original.file_stem().unwrap_or_default();
        let ext = original.extension();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        
        // Agregar UUID corto para unicidad garantizada
        let uuid_short = uuid::Uuid::new_v4()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        
        let new_name = format!(
            "{}_conflicto_{}_{}", 
            stem.to_string_lossy(),
            timestamp,
            uuid_short
        );
        
        let mut path = original.parent().unwrap().to_path_buf();
        path.push(new_name);
        if let Some(e) = ext {
            path.set_extension(e);
        }
        path
    }
}
```

**Tests Requeridos:**
- `test_conflict_names_never_collide`
- `test_concurrent_conflict_naming`
- `test_special_characters_in_name`

---

> [!NOTE]
> Para la matriz completa de riesgos y simulaciones, ver:
> - [TRACE-risks-mitigations.md](../.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md)
>
> Diagrama de secuencia relacionado:
> - [SEQ-004-conflict-resolution.puml](../.devtrail/02-design/diagrams/SEQ-004-conflict-resolution.puml)

---

## Ver tambien

- [[04-Componentes/07-motor-sincronizacion|Motor de Sincronizacion]] - Estado de conflictos en el motor
- [[04-Componentes/12-auditoria|Auditoria]] - Registro de resoluciones de conflictos
- [[03-Arquitectura/03-adaptadores-entrada|Adaptadores de Entrada]] - UI intercambiables

