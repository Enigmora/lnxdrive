# Multi-Cuenta con Namespaces

> **Ubicación:** `07-Extensibilidad/03-multi-cuenta-namespaces.md`
> **Relacionado:** [Puerto ICloudProvider](02-puerto-icloudprovider.md), [Configuración YAML](../05-Implementacion/05-configuracion-yaml.md)

---

## Parte XIII: Multi-Cuenta con Namespaces

### 13.1 Necesidad de Multiples Cuentas

El soporte para multiples cuentas simultaneas es una **necesidad real** de los usuarios:

| Escenario | Configuracion Tipica |
|-----------|---------------------|
| Trabajo + Personal | 2 cuentas OneDrive (corporativa + personal) |
| Freelancer/Consultor | 3+ cuentas (una por cliente/proyecto) |
| Familia Compartida | Cuenta propia + acceso a cuenta familiar |
| Migracion de Cuenta | Cuenta antigua + cuenta nueva temporalmente |
| Educacion | Cuenta institucional (.edu) + personal |
| Multi-cloud | OneDrive + Google Drive + Dropbox |

**Referencia de mercado:** Insync (cliente comercial) soporta cuentas ilimitadas y es uno de sus principales diferenciadores. El cliente oficial de Windows tambien soporta multiples cuentas de OneDrive.

### 13.2 Modelo de Namespaces

Cada cuenta se identifica con un **namespace** unico que combina proveedor y alias:

```
┌─────────────────────────────────────────────────────────────────────┐
│  ESTRUCTURA DE NAMESPACES                                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Formato: {provider}:{alias}                                        │
│                                                                     │
│  Ejemplos:                                                          │
│  • onedrive:personal      → Cuenta personal de OneDrive             │
│  • onedrive:trabajo       → Cuenta corporativa de OneDrive          │
│  • onedrive:cliente-acme  → OneDrive del cliente ACME               │
│  • gdrive:principal       → Cuenta principal de Google Drive        │
│  • dropbox:default        → Cuenta de Dropbox                       │
│  • nextcloud:servidor-nas → Nextcloud auto-hospedado                │
│                                                                     │
│  Vista en filesystem:                                               │
│  ─────────────────────────────────────────────────────────────────  │
│                                                                     │
│  ~/CloudSync/                                                       │
│  ├── onedrive-personal/       ← namespace: onedrive:personal        │
│  │   ├── Documents/                                                 │
│  │   ├── Pictures/                                                  │
│  │   └── ...                                                        │
│  ├── onedrive-trabajo/        ← namespace: onedrive:trabajo         │
│  │   ├── Proyectos/                                                 │
│  │   └── ...                                                        │
│  ├── gdrive-principal/        ← namespace: gdrive:principal         │
│  │   └── ...                                                        │
│  └── dropbox/                 ← namespace: dropbox:default          │
│      └── ...                                                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 13.3 Configuracion Multi-Cuenta

```yaml
# ~/.config/lnxdrive/config.yaml
lnxdrive:
  version: 1

  # Directorio raiz para todas las cuentas
  sync_root: ~/CloudSync

  # Definicion de cuentas
  accounts:
    # ─── OneDrive Personal ────────────────────────────────────────────
    - id: "onedrive:personal"
      provider: onedrive
      alias: "Personal"
      email: "usuario@outlook.com"
      root: ~/CloudSync/onedrive-personal
      enabled: true

      # Configuracion especifica de esta cuenta
      sync:
        direction: bidirectional
        exclude:
          - "**/node_modules/**"
          - "**/.git/**"

    # ─── OneDrive Trabajo ─────────────────────────────────────────────
    - id: "onedrive:trabajo"
      provider: onedrive
      alias: "Trabajo"
      email: "usuario@empresa.com"
      root: ~/CloudSync/onedrive-trabajo
      enabled: true

      # Configuracion corporativa
      sync:
        direction: bidirectional
        # Politica de archivos grandes para evitar saturar VPN
        large_files:
          schedule: off_peak
          off_peak_hours: "22:00-06:00"

    # ─── Google Drive ─────────────────────────────────────────────────
    - id: "gdrive:principal"
      provider: google-drive
      alias: "Google"
      email: "usuario@gmail.com"
      root: ~/CloudSync/gdrive
      enabled: true

    # ─── Dropbox ──────────────────────────────────────────────────────
    - id: "dropbox:default"
      provider: dropbox
      alias: "Dropbox"
      email: "usuario@email.com"
      root: ~/CloudSync/dropbox
      enabled: false  # Deshabilitada temporalmente

  # Configuracion global que aplica a todas las cuentas
  global:
    notifications:
      on_conflict: always
      on_error: always
```

### 13.4 Arquitectura Interna Multi-Cuenta

```
┌─────────────────────────────────────────────────────────────────────┐
│  ARQUITECTURA MULTI-CUENTA                                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                       ┌─────────────────┐                           │
│                       │  lnxdrive-daemon  │                           │
│                       │   (proceso unico)│                          │
│                       └────────┬────────┘                           │
│                                │                                    │
│              ┌─────────────────┼─────────────────┐                  │
│              │                 │                 │                  │
│              ▼                 ▼                 ▼                  │
│     ┌────────────────┐ ┌────────────────┐ ┌────────────────┐        │
│     │ AccountManager │ │ AccountManager │ │ AccountManager │        │
│     │ onedrive:      │ │ onedrive:      │ │ gdrive:        │        │
│     │ personal       │ │ trabajo        │ │ principal      │        │
│     └───────┬────────┘ └───────┬────────┘ └───────┬────────┘        │
│             │                  │                  │                 │
│             ▼                  ▼                  ▼                 │
│     ┌────────────────┐ ┌────────────────┐ ┌────────────────┐        │
│     │  SyncEngine    │ │  SyncEngine    │ │  SyncEngine    │        │
│     │  (instancia)   │ │  (instancia)   │ │  (instancia)   │        │
│     └───────┬────────┘ └───────┬────────┘ └───────┬────────┘        │
│             │                  │                  │                 │
│             └──────────────────┼──────────────────┘                 │
│                                │                                    │
│                    ┌───────────▼───────────┐                        │
│                    │   Shared Resources    │                        │
│                    │ • HTTP Connection Pool│                        │
│                    │ • Global Rate Limiter │                        │
│                    │ • SQLite State DB     │                        │
│                    │ • FUSE Daemon         │                        │
│                    │ • DBus Service        │                        │
│                    └───────────────────────┘                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Principios de diseno:**
- **Un solo daemon** para todas las cuentas (no N procesos)
- **Recursos compartidos** para eficiencia (pool HTTP, rate limiter global)
- **Aislamiento logico** entre cuentas (cada una tiene su SyncEngine)
- **Base de datos unificada** con particion por namespace

### 13.5 Impacto en Recursos

```
┌─────────────────────────────────────────────────────────────────────┐
│  ANALISIS DE CONSUMO DE RECURSOS                                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  MEMORIA RAM:                                                       │
│  ─────────────────────────────────────────────────────────────────  │
│  • Daemon base:                    ~30-50 MB                        │
│  • Por cuenta adicional:           +5-15 MB                         │
│    (tokens OAuth, cache delta, metadata en memoria)                 │
│  • Cache de placeholders:          +2-5 MB por 10K archivos         │
│                                                                     │
│  Ejemplos:                                                          │
│  • 1 cuenta OneDrive:              ~40-60 MB                        │
│  • 3 cuentas (mixed providers):    ~60-90 MB                        │
│  • 5 cuentas + 100K archivos:      ~100-150 MB                      │
│                                                                     │
│  CPU:                                                               │
│  ─────────────────────────────────────────────────────────────────  │
│  • Polling delta (por cuenta):     ~0.1% cada 30s-5min              │
│  • Sync activo:                    Variable segun transferencias    │
│  • Estado idle:                    ~0% (event-driven)               │
│                                                                     │
│  OPTIMIZACIONES IMPLEMENTADAS:                                      │
│  ─────────────────────────────────────────────────────────────────  │
│  ✓ Un daemon para todas las cuentas (no N procesos)                 │
│  ✓ Pool de conexiones HTTP compartido                               │
│  ✓ Polling escalonado (no todas las cuentas a la vez)               │
│  ✓ Delta sync reduce trafico 90%+ vs full scan                      │
│  ✓ Prioriza cuenta con actividad reciente del usuario               │
│                                                                     │
│  CONCLUSION:                                                        │
│  El impacto es MINIMO si se disena correctamente. El cuello de      │
│  botella real es I/O de disco y red, no RAM ni CPU.                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 13.6 CLI con Soporte Multi-Cuenta

```bash
# ─── Gestion de Cuentas ───────────────────────────────────────────────

# Anadir nueva cuenta (inicia flujo OAuth interactivo)
lnxdrive account add --provider onedrive --alias "MiCuenta"
lnxdrive account add --provider google-drive --alias "Google"

# Listar cuentas configuradas
lnxdrive account list
# Output:
# ID                    Provider      Alias       Status    Files
# onedrive:personal     OneDrive      Personal    ✓ Sync    12,456
# onedrive:trabajo      OneDrive      Trabajo     ✓ Sync    3,891
# gdrive:principal      Google Drive  Google      ✓ Sync    8,234
# dropbox:default       Dropbox       Dropbox     ○ Disabled  -

# Informacion detallada de una cuenta
lnxdrive account info onedrive:personal
# Output:
# Account: onedrive:personal
# Provider: OneDrive
# Email: usuario@outlook.com
# Root: ~/CloudSync/onedrive-personal
# Status: Syncing
# Files: 12,456 (234 hydrated, 12,222 online-only)
# Space: 45.2 GB / 100 GB (45%)
# Last Sync: 2 minutes ago

# Habilitar/deshabilitar cuenta
lnxdrive account enable dropbox:default
lnxdrive account disable onedrive:trabajo

# Eliminar cuenta (mantiene archivos locales)
lnxdrive account remove gdrive:backup --keep-files

# ─── Operaciones por Cuenta ───────────────────────────────────────────

# Estado de cuenta especifica
lnxdrive status onedrive:personal

# Sincronizar cuenta especifica
lnxdrive sync onedrive:trabajo

# Conflictos de cuenta especifica
lnxdrive conflicts onedrive:personal

# ─── Operaciones Globales ─────────────────────────────────────────────

# Estado de todas las cuentas
lnxdrive status --all

# Sincronizar todas las cuentas habilitadas
lnxdrive sync --all

# Pausar/reanudar sincronizacion global
lnxdrive pause --all
lnxdrive resume --all

# ─── Operaciones con Archivos ─────────────────────────────────────────

# El path determina automaticamente la cuenta
lnxdrive hydrate ~/CloudSync/onedrive-personal/documento.pdf
lnxdrive pin ~/CloudSync/gdrive-principal/importante/

# O especificar explicitamente
lnxdrive hydrate onedrive:trabajo:/Proyectos/archivo.zip
```

### 13.7 DBus API Multi-Cuenta

```xml
<!-- org.enigmora.LNXDrive.Accounts -->
<interface name="org.enigmora.LNXDrive.Accounts">
  <!-- Lista todas las cuentas configuradas -->
  <method name="List">
    <arg type="a(ssssb)" direction="out" name="accounts"/>
    <!-- Array of (id, provider, alias, email, enabled) -->
  </method>

  <!-- Obtiene informacion detallada de una cuenta -->
  <method name="GetInfo">
    <arg type="s" direction="in" name="account_id"/>
    <arg type="a{sv}" direction="out" name="info"/>
  </method>

  <!-- Inicia autenticacion para nueva cuenta -->
  <method name="Add">
    <arg type="s" direction="in" name="provider"/>
    <arg type="s" direction="in" name="alias"/>
    <arg type="s" direction="out" name="auth_url"/>
  </method>

  <!-- Habilita/deshabilita una cuenta -->
  <method name="SetEnabled">
    <arg type="s" direction="in" name="account_id"/>
    <arg type="b" direction="in" name="enabled"/>
  </method>

  <!-- Senales -->
  <signal name="AccountAdded">
    <arg type="s" name="account_id"/>
  </signal>

  <signal name="AccountRemoved">
    <arg type="s" name="account_id"/>
  </signal>

  <signal name="AccountStatusChanged">
    <arg type="s" name="account_id"/>
    <arg type="s" name="old_status"/>
    <arg type="s" name="new_status"/>
  </signal>
</interface>
```

---

## Ver tambien

- [Arquitectura Multi-Proveedor](01-arquitectura-multi-proveedor.md) - Vision general de extensibilidad
- [Puerto ICloudProvider](02-puerto-icloudprovider.md) - Definicion del puerto y adaptadores
- [Artefactos Reutilizables](04-artefactos-reutilizables.md) - Componentes extraibles como librerias
- [CLI](../04-Componentes/06-cli.md) - Referencia completa de comandos
