# CLI Universal

> **Ubicación:** `04-Componentes/06-cli.md`
> **Relacionado:** [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md)

---

### 4.6 CLI Universal

```bash
# Operaciones basicas
lnxdrive sync                      # Sincronizar ahora
lnxdrive status                    # Estado actual
lnxdrive status --json             # Output estructurado para scripts
lnxdrive status ~/OneDrive/file.txt # Estado de archivo especifico

# Files on Demand
lnxdrive hydrate ~/OneDrive/big-file.zip    # Descargar archivo
lnxdrive dehydrate ~/OneDrive/old-project/  # Liberar espacio
lnxdrive pin ~/OneDrive/important/          # Mantener siempre offline

# Conflictos
lnxdrive conflicts                 # Listar conflictos
lnxdrive conflicts resolve abc123 --keep-local
lnxdrive conflicts resolve abc123 --keep-remote
lnxdrive conflicts resolve abc123 --keep-both
lnxdrive conflicts preview abc123  # Ver diff antes de resolver

# Auditoria y explicabilidad
lnxdrive explain ~/OneDrive/file.txt        # ¿Por que este estado?
lnxdrive audit --since "2 hours ago"        # Historial de acciones
lnxdrive audit ~/OneDrive/file.txt          # Historial de archivo especifico

# Metricas
lnxdrive metrics                   # Estadisticas de sincronizacion
lnxdrive metrics --prometheus      # Formato Prometheus
lnxdrive metrics --json            # Formato JSON

# Configuracion
lnxdrive config show               # Mostrar configuracion actual
lnxdrive config set conflicts.default-strategy keep-local
lnxdrive config validate           # Validar archivo YAML
# Reportes de telemetría
lnxdrive report list               # Ver reportes pendientes
lnxdrive report view <id>          # Ver contenido de un reporte
lnxdrive report send <id>          # Enviar reporte específico
lnxdrive report send --all         # Enviar todos los pendientes
lnxdrive report delete <id>        # Eliminar sin enviar
lnxdrive report delete --all       # Eliminar todos
```

---

## Ver tambien

- [Files-on-Demand FUSE](01-files-on-demand-fuse.md) - Sistema de archivos bajo demanda
- [Adaptador GNOME](02-ui-gnome.md) - Integracion con GNOME
- [Adaptador KDE Plasma](03-ui-kde-plasma.md) - Integracion con KDE
- [Adaptador GTK3](04-ui-gtk3.md) - Integracion con XFCE/MATE
- [Telemetría](13-telemetria.md) - Sistema de reportes de errores
