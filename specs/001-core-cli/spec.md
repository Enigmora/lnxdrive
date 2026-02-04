# Feature Specification: Core + CLI (Fase 1 - Fundamentos)

**Feature Branch**: `001-core-cli`
**Created**: 2026-02-03
**Status**: Draft
**Input**: Motor de sincronizacion fundacional, autenticacion OAuth2, integracion Microsoft Graph, y CLI completa para LNXDrive

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Primera Autenticacion con OneDrive (Priority: P1)

Como usuario de Linux que quiere sincronizar sus archivos de OneDrive, necesito poder autenticarme con mi cuenta Microsoft de forma segura para que el sistema pueda acceder a mis archivos en la nube.

**Why this priority**: Sin autenticacion, ninguna otra funcionalidad del sistema es posible. Es el prerequisito fundamental para cualquier operacion de sincronizacion.

**Independent Test**: Se puede verificar completamente ejecutando `lnxdrive auth login` y confirmando que el sistema obtiene y almacena tokens validos en el keyring del sistema.

**Acceptance Scenarios**:

1. **Given** un usuario sin autenticacion previa, **When** ejecuta `lnxdrive auth login`, **Then** el sistema abre el navegador con la pagina de login de Microsoft, el usuario se autentica, y los tokens se almacenan de forma segura en el keyring del sistema.

2. **Given** un usuario con tokens expirados, **When** el sistema necesita acceder a la API, **Then** los tokens se refrescan automaticamente sin intervencion del usuario.

3. **Given** un usuario con tokens invalidos (revocados), **When** el sistema detecta el error, **Then** notifica al usuario que debe re-autenticarse y proporciona instrucciones claras.

4. **Given** un usuario autenticado, **When** ejecuta `lnxdrive auth logout`, **Then** los tokens se eliminan del keyring y el sistema confirma la desconexion.

5. **Given** un usuario autenticado, **When** ejecuta `lnxdrive auth status`, **Then** el sistema muestra informacion de la cuenta conectada (email, espacio usado/disponible).

---

### User Story 2 - Sincronizacion Inicial de Archivos (Priority: P1)

Como usuario autenticado, necesito poder sincronizar mi carpeta de OneDrive por primera vez para tener acceso local a mis archivos y que los cambios se reflejen en ambas direcciones.

**Why this priority**: La sincronizacion inicial es la funcionalidad core del producto. Sin ella, el sistema no cumple su proposito fundamental.

**Independent Test**: Se puede verificar creando un archivo local en la carpeta de sincronizacion y confirmando que aparece en OneDrive web, y viceversa.

**Acceptance Scenarios**:

1. **Given** un usuario autenticado sin sincronizacion previa, **When** ejecuta `lnxdrive sync`, **Then** el sistema descarga la estructura de directorios y metadatos de OneDrive, creando la carpeta local de sincronizacion.

2. **Given** un usuario con archivos locales nuevos en la carpeta de sincronizacion, **When** se ejecuta la sincronizacion, **Then** los archivos se suben a OneDrive manteniendo la estructura de directorios.

3. **Given** un usuario con archivos modificados remotamente, **When** se ejecuta la sincronizacion, **Then** los archivos locales se actualizan con las versiones remotas mas recientes.

4. **Given** un archivo que se esta sincronizando, **When** ocurre un error de red transitorio, **Then** el sistema reintenta automaticamente con backoff exponencial y registra el evento en el audit log.

5. **Given** una sincronizacion en progreso, **When** el usuario ejecuta `lnxdrive status`, **Then** puede ver el progreso actual (archivos procesados, pendientes, errores).

---

### User Story 3 - Sincronizacion Delta Incremental (Priority: P2)

Como usuario con sincronizacion establecida, necesito que el sistema detecte y sincronice solo los cambios incrementales para optimizar el uso de ancho de banda y reducir el tiempo de sincronizacion.

**Why this priority**: Una vez establecida la sincronizacion inicial, la eficiencia del sistema depende de la capacidad de detectar cambios incrementales. Esto es critico para el uso diario pero secundario al establecimiento inicial.

**Independent Test**: Se puede verificar modificando un unico archivo pequeno y confirmando que solo ese archivo se transfiere, no toda la coleccion.

**Acceptance Scenarios**:

1. **Given** una sincronizacion ya completada, **When** se modifica un archivo en OneDrive, **Then** el sistema detecta el cambio mediante Delta API y sincroniza solo ese archivo.

2. **Given** una sincronizacion ya completada, **When** se modifica un archivo localmente, **Then** el sistema detecta el cambio y sube solo ese archivo a OneDrive.

3. **Given** multiples cambios remotos, **When** se ejecuta la sincronizacion, **Then** el sistema procesa todos los cambios en orden cronologico usando delta tokens.

4. **Given** un delta token valido almacenado, **When** el sistema consulta cambios, **Then** la API retorna solo los cambios desde el ultimo sync, no la coleccion completa.

5. **Given** un delta token expirado (>90 dias inactivo), **When** el sistema intenta usarlo, **Then** detecta el error y ejecuta una re-sincronizacion completa notificando al usuario.

---

### User Story 4 - Observacion de Cambios Locales en Tiempo Real (Priority: P2)

Como usuario, necesito que el sistema detecte automaticamente cuando creo, modifico o elimino archivos locales para sincronizarlos sin intervencion manual.

**Why this priority**: La deteccion automatica de cambios es esencial para una experiencia de usuario fluida, pero puede implementarse despues de tener sincronizacion manual funcional.

**Independent Test**: Se puede verificar creando un archivo en la carpeta sincronizada y observando que aparece en OneDrive sin ejecutar comandos adicionales.

**Acceptance Scenarios**:

1. **Given** el daemon ejecutandose, **When** el usuario crea un archivo en la carpeta de sincronizacion, **Then** el sistema detecta el cambio y programa la sincronizacion automaticamente.

2. **Given** el daemon ejecutandose, **When** el usuario modifica un archivo existente, **Then** el sistema detecta la modificacion y sincroniza solo los cambios.

3. **Given** el daemon ejecutandose, **When** el usuario elimina un archivo, **Then** el sistema sincroniza la eliminacion a OneDrive (moviendo a papelera si esta configurado).

4. **Given** multiples cambios rapidos al mismo archivo, **When** el sistema procesa los eventos, **Then** agrupa las modificaciones y sube solo la version final (debouncing).

5. **Given** un archivo que esta siendo modificado continuamente (editor abierto), **When** el sistema lo detecta, **Then** espera a que el archivo este estable antes de sincronizar.

---

### User Story 5 - CLI para Estado y Diagnostico (Priority: P2)

Como usuario, necesito poder consultar el estado de sincronizacion y obtener informacion de diagnostico a traves de la linea de comandos para entender que esta pasando con mis archivos.

**Why this priority**: La capacidad de diagnostico es fundamental para la usabilidad, pero el sistema puede funcionar sin ella inicialmente.

**Independent Test**: Se puede verificar ejecutando `lnxdrive status` y obteniendo informacion coherente sobre el estado del sistema.

**Acceptance Scenarios**:

1. **Given** una sincronizacion en curso, **When** el usuario ejecuta `lnxdrive status`, **Then** ve un resumen del estado actual (archivos sincronizados, pendientes, errores, ultima sincronizacion).

2. **Given** un archivo especifico, **When** el usuario ejecuta `lnxdrive status <path>`, **Then** ve el estado detallado de ese archivo (sincronizado, pendiente, conflicto, error).

3. **Given** cualquier estado del sistema, **When** el usuario ejecuta `lnxdrive status --json`, **Then** obtiene la informacion en formato JSON estructurado para uso programatico.

4. **Given** un error de sincronizacion, **When** el usuario ejecuta `lnxdrive explain <path>`, **Then** recibe una explicacion clara y legible de por que el archivo esta en ese estado y que puede hacer.

5. **Given** actividad reciente del sistema, **When** el usuario ejecuta `lnxdrive audit --since "1 hour ago"`, **Then** ve un historial de acciones realizadas con contexto completo.

---

### User Story 6 - Rate Limiting y Respeto de Cuotas (Priority: P3)

Como usuario, necesito que el sistema respete los limites de la API de Microsoft Graph para evitar bloqueos y garantizar una sincronizacion confiable a largo plazo.

**Why this priority**: El rate limiting es critico para la estabilidad, pero la implementacion basica puede funcionar inicialmente con limites conservadores.

**Independent Test**: Se puede verificar realizando multiples operaciones simultaneas y confirmando que el sistema no recibe errores 429 (Too Many Requests).

**Acceptance Scenarios**:

1. **Given** multiples archivos pendientes de sincronizacion, **When** el sistema los procesa, **Then** respeta los limites de concurrencia configurados por tipo de operacion.

2. **Given** una respuesta 429 de la API, **When** el sistema la recibe, **Then** respeta el header Retry-After, registra el evento, y reintenta despues del tiempo indicado.

3. **Given** errores 429 recurrentes, **When** el sistema los detecta, **Then** reduce automaticamente su tasa de requests por un periodo (adaptive throttling).

4. **Given** un periodo sin errores 429, **When** el sistema lo detecta, **Then** aumenta gradualmente su tasa de requests hasta el limite configurado.

5. **Given** una sincronizacion inicial con miles de archivos, **When** el sistema la ejecuta, **Then** activa automaticamente el modo bulk con limites mas conservadores.

---

### User Story 7 - Servicio Daemon Persistente (Priority: P3)

Como usuario, necesito que el sistema de sincronizacion se ejecute automaticamente al iniciar sesion y permanezca activo en segundo plano para mantener mis archivos sincronizados sin intervencion manual.

**Why this priority**: El daemon es necesario para la experiencia completa, pero la CLI puede funcionar de forma independiente para el MVP inicial.

**Independent Test**: Se puede verificar reiniciando la sesion y confirmando que el daemon se inicia automaticamente y comienza a sincronizar.

**Acceptance Scenarios**:

1. **Given** el servicio instalado, **When** el usuario inicia sesion en su escritorio, **Then** el daemon se inicia automaticamente como servicio de usuario systemd.

2. **Given** el daemon en ejecucion, **When** el usuario ejecuta `lnxdrive daemon status`, **Then** puede ver que el servicio esta activo, su PID, tiempo de ejecucion y uso de recursos.

3. **Given** el daemon en ejecucion, **When** el usuario ejecuta `lnxdrive daemon stop`, **Then** el servicio se detiene limpiamente completando operaciones pendientes.

4. **Given** el daemon detenido, **When** el usuario ejecuta `lnxdrive daemon start`, **Then** el servicio se inicia y comienza a sincronizar.

5. **Given** un crash del daemon, **When** systemd lo detecta, **Then** reinicia el servicio automaticamente y registra el evento.

---

### Edge Cases

- **Sin conexion a internet**: El sistema debe encolar operaciones locales y sincronizarlas cuando se restaure la conectividad.
- **Disco lleno**: El sistema debe detectar espacio insuficiente antes de descargar archivos grandes y notificar al usuario.
- **Archivo en uso por otra aplicacion**: El sistema debe detectar archivos bloqueados y reintentar mas tarde.
- **Token delta expirado**: El sistema debe manejar gracefully la expiracion ejecutando resync completo.
- **Archivos con caracteres especiales**: El sistema debe manejar nombres con unicode, espacios, y caracteres especiales correctamente.
- **Archivos muy grandes (>4GB)**: El sistema debe usar upload sessions con chunks y soportar resume tras interrupciones.
- **Cambios conflictivos simultaneos**: Cuando el mismo archivo cambia local y remotamente, el sistema debe detectar el conflicto y marcarlo para resolucion manual (Fase 5).
- **Carpeta de sincronizacion eliminada**: El sistema debe detectar la eliminacion y preguntar al usuario si desea recrearla o desvincular la cuenta.
- **Multiples instancias**: El sistema debe prevenir que multiples instancias del daemon corran simultaneamente.

## Requirements *(mandatory)*

### Functional Requirements

**Autenticacion:**
- **FR-001**: El sistema DEBE autenticar usuarios mediante OAuth2 Authorization Code + PKCE sin client_secret (aplicacion publica).
- **FR-002**: El sistema DEBE almacenar tokens de acceso y refresh en el keyring del sistema (libsecret en Linux).
- **FR-003**: El sistema DEBE refrescar tokens automaticamente antes de su expiracion.
- **FR-004**: El sistema DEBE solicitar los scopes minimos necesarios: `Files.ReadWrite`, `offline_access`, `User.Read`.
- **FR-005**: El sistema DEBE soportar un App ID embebido por defecto con opcion de usar App ID personalizado.

**Sincronizacion:**
- **FR-006**: El sistema DEBE sincronizar archivos bidireccionalmente entre el sistema de archivos local y OneDrive.
- **FR-007**: El sistema DEBE usar Delta API para detectar cambios incrementales en lugar de enumerar toda la coleccion.
- **FR-008**: El sistema DEBE persistir el delta token entre reinicios para continuar desde el ultimo estado conocido.
- **FR-009**: El sistema DEBE soportar subida directa (PUT) para archivos <4MB y upload sessions para archivos mayores.
- **FR-010**: El sistema DEBE verificar integridad de archivos mediante hashes antes de marcar como sincronizados.
- **FR-011**: El sistema DEBE detectar cambios locales mediante observacion del sistema de archivos (inotify).

**CLI:**
- **FR-012**: El sistema DEBE proveer comandos para: `auth`, `sync`, `status`, `explain`, `audit`, `config`, `daemon`.
- **FR-013**: El sistema DEBE soportar output JSON estructurado para todos los comandos con flag `--json`.
- **FR-014**: El sistema DEBE mostrar progreso de sincronizacion con indicadores claros de archivos procesados.

**Rate Limiting:**
- **FR-015**: El sistema DEBE implementar token bucket por tipo de endpoint con limites configurables.
- **FR-016**: El sistema DEBE respetar headers Retry-After en respuestas 429.
- **FR-017**: El sistema DEBE adaptar automaticamente los limites basandose en errores 429 recibidos.

**Daemon:**
- **FR-018**: El sistema DEBE ejecutarse como servicio de usuario systemd.
- **FR-019**: El sistema DEBE exponer una API D-Bus para comunicacion con interfaces graficas futuras.
- **FR-020**: El sistema DEBE reiniciarse automaticamente tras crashes (RestartOnFailure).

**Estado y Persistencia:**
- **FR-021**: El sistema DEBE persistir el estado de sincronizacion en una base de datos local.
- **FR-022**: El sistema DEBE mantener un registro de auditoria de todas las operaciones con timestamps y contexto.

**Configuracion:**
- **FR-023**: El sistema DEBE leer configuracion desde archivo YAML en `~/.config/lnxdrive/config.yaml`.
- **FR-024**: El sistema DEBE validar la configuracion al iniciar y reportar errores claros.
- **FR-025**: El sistema DEBE usar defaults sensatos cuando la configuracion no especifica valores.

### Key Entities

- **SyncItem**: Representa un archivo o directorio sincronizado. Contiene: identificador unico, ruta local, ruta remota, estado actual (Online, Hydrating, Hydrated, Modified, Conflicted, Error), hash de contenido, timestamp de ultima sincronizacion, y metadatos.

- **Account**: Representa una cuenta de OneDrive vinculada. Contiene: identificador, email del usuario, tokens de autenticacion (referencia a keyring), espacio utilizado/disponible, y estado de conexion.

- **SyncSession**: Representa una sesion de sincronizacion activa. Contiene: timestamp de inicio, archivos procesados, archivos pendientes, errores encontrados, y delta token actual.

- **AuditEntry**: Representa un registro de auditoria. Contiene: timestamp, accion realizada, item afectado, resultado (exito/error con razon), y contexto adicional como mapas clave-valor.

- **Conflict**: Representa un conflicto de sincronizacion detectado. Contiene: item en conflicto, version local, version remota, timestamp de deteccion, y resolucion aplicada (si alguna).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Un usuario puede completar la autenticacion inicial con OneDrive en menos de 2 minutos desde la ejecucion del comando.

- **SC-002**: La sincronizacion inicial de 1000 archivos pequenos (<1MB cada uno) se completa en menos de 10 minutos con conexion estable.

- **SC-003**: Los cambios locales a un archivo se reflejan en OneDrive web en menos de 30 segundos cuando el daemon esta activo.

- **SC-004**: Los cambios remotos se detectan y descargan en menos de 60 segundos (dependiente del intervalo de polling).

- **SC-005**: El sistema mantiene 0 errores 429 (throttling) durante operaciones normales (no bulk) en 95% de las sesiones.

- **SC-006**: El daemon consume menos de 50MB de memoria en reposo sin archivos pendientes.

- **SC-007**: El daemon consume menos del 1% de CPU en reposo, y menos del 10% durante sincronizacion activa.

- **SC-008**: El comando `lnxdrive explain <path>` proporciona una explicacion comprensible para cualquier estado de archivo en menos de 1 segundo.

- **SC-009**: El sistema puede recuperarse automaticamente de errores de red transitorios sin intervencion del usuario en 100% de los casos.

- **SC-010**: El audit log registra 100% de las operaciones de sincronizacion con contexto suficiente para diagnostico.

- **SC-011**: Un usuario puede entender el estado actual del sistema ejecutando `lnxdrive status` y obteniendo informacion clara en menos de 2 segundos.

## Assumptions

- El usuario tiene acceso a una cuenta Microsoft con OneDrive personal o empresarial.
- El sistema operativo es Linux con systemd como init system.
- El sistema tiene libsecret instalado para almacenamiento seguro de credenciales.
- El usuario tiene permisos de escritura en `~/.config/lnxdrive/` y en la carpeta de sincronizacion.
- La conectividad a internet es intermitente pero generalmente disponible.
- Los archivos sincronizados son menores a 250GB (limite de OneDrive para archivos individuales).
- El sistema de archivos local soporta extended attributes para metadatos.

## Out of Scope

- **Files-on-Demand (FUSE)**: Se implementara en Fase 2.
- **Interfaces graficas (GNOME, KDE)**: Se implementaran en Fases 3 y 7.
- **Resolucion visual de conflictos**: Se implementara en Fase 5; esta fase solo detecta y marca conflictos.
- **Multi-cuenta**: Se implementara en Fase 6; esta fase soporta una sola cuenta.
- **Otros proveedores cloud (Google Drive, Dropbox)**: Se implementaran en Fase 8.
- **Telemetria y metricas Prometheus**: Se implementara en Fase 4.
- **SharePoint y carpetas compartidas**: Se implementara en Fase 9.

## Dependencies

- **Microsoft Graph API v1.0**: Para operaciones de OneDrive.
- **libsecret**: Para almacenamiento seguro de tokens.
- **systemd**: Para gestion del daemon como servicio de usuario.
- **D-Bus**: Para IPC con futuras interfaces graficas.
- **Base de datos local**: Para persistencia de estado (SQLite asumido por convencion del proyecto).
