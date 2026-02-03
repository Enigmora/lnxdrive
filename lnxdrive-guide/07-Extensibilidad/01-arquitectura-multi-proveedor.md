# Arquitectura Multi-Proveedor

> **Ubicacion:** `07-Extensibilidad/01-arquitectura-multi-proveedor.md`
> **Relacionado:** [Puerto ICloudProvider](02-puerto-icloudprovider.md), [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md)

---

## Parte XII: Extensibilidad Multi-Proveedor

### 12.1 Arquitectura Agnostica al Proveedor

La arquitectura hexagonal de LNXDrive permite extender el sistema a **multiples proveedores de almacenamiento en la nube** sin modificar el nucleo de sincronizacion. El puerto `ICloudProvider` actua como contrato que cualquier proveedor debe implementar.

```
┌─────────────────────────────────────────────────────────────────────┐
│  NUCLEO AGNOSTICO + ADAPTADORES DE PROVEEDOR                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                      ┌─────────────────────┐                        │
│                      │     NIMBUS CORE     │                        │
│                      │   (sync engine)     │                        │
│                      │                     │                        │
│                      │  • State Machine    │                        │
│                      │  • Conflict Engine  │                        │
│                      │  • Audit System     │                        │
│                      │  • Rate Limiter     │                        │
│                      └──────────┬──────────┘                        │
│                                 │                                   │
│                    ┌────────────┼────────────┐                      │
│                    │            │            │                      │
│                    ▼            ▼            ▼                      │
│            ┌────────────┐ ┌────────────┐ ┌────────────┐             │
│            │  OneDrive  │ │Google Drive│ │  Dropbox   │             │
│            │  Adapter   │ │  Adapter   │ │  Adapter   │             │
│            │            │ │            │ │            │             │
│            │ MS Graph   │ │ Drive API  │ │ Dropbox    │             │
│            │ API v1.0   │ │ v3         │ │ API v2     │             │
│            └────────────┘ └────────────┘ └────────────┘             │
│                    │            │            │                      │
│                    ▼            ▼            ▼                      │
│            ┌────────────┐ ┌────────────┐ ┌────────────┐             │
│            │  Nextcloud │ │   pCloud   │ │   Proton   │             │
│            │  Adapter   │ │  Adapter   │ │   Drive    │             │
│            │            │ │            │ │  Adapter   │             │
│            │  WebDAV    │ │ pCloud API │ │ Proton API │             │
│            └────────────┘ └────────────┘ └────────────┘             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Ver tambien

- [Puerto ICloudProvider](02-puerto-icloudprovider.md) - Definicion del puerto y adaptadores
- [Multi-Cuenta con Namespaces](03-multi-cuenta-namespaces.md) - Soporte para multiples cuentas
- [Artefactos Reutilizables](04-artefactos-reutilizables.md) - Componentes extraibles como librerias
- [Arquitectura Hexagonal](../03-Arquitectura/01-arquitectura-hexagonal.md) - Fundamentos arquitectonicos
