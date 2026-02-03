# Adaptador XFCE/MATE (GTK3)

> **Ubicación:** `04-Componentes/04-ui-gtk3.md`
> **Relacionado:** [Adaptadores de Entrada](../03-Arquitectura/03-adaptadores-entrada.md)

---

### 4.5 Adaptador XFCE/MATE (GTK3)

```
┌─────────────────────────────────────────────────────────────────┐
│  lnxdrive-xfce                                                     │
│  ─────────────────────────────────────────────────────────────   │
│                                                                  │
│  Componentes:                                                    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Thunar Custom Actions                                    │    │
│  │  • Configuracion via GUI de Thunar                       │    │
│  │  • Acciones personalizadas para sync                      │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  MATE/Caja Extension                                      │    │
│  │  • Similar a Nautilus (comparten codigo base)            │    │
│  │  • Emblems via GIO                                        │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  System Tray (XEmbed/SNI)                                 │    │
│  │  • Icono tradicional de bandeja                          │    │
│  │  • Menu con GTK3                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Aplicacion de Preferencias Standalone                    │    │
│  │  • GTK3 simple y ligera                                   │    │
│  │  • Para entornos minimalistas                             │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Files-on-Demand FUSE](01-files-on-demand-fuse.md) - Sistema de archivos bajo demanda
- [Adaptador GNOME](02-ui-gnome.md) - Integracion con GNOME
- [Adaptador Cosmic](05-ui-cosmic.md) - Integracion con Cosmic
- [CLI Universal](06-cli.md) - Interfaz de linea de comandos
