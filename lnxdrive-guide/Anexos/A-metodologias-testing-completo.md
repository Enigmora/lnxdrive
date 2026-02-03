# Anexo A: Metodologías de Testing y Depuración para LNXDrive

> **Ubicación:** `Anexos/A-metodologias-testing-completo.md`
> **Relacionado:** [Estrategia de Testing](../06-Testing/01-estrategia-testing.md), [Testing de FUSE](../06-Testing/03-testing-fuse.md)

---

> **Anexo de:** Propuesta Arquitectónica: Cliente OneDrive para Linux (`Propuesta-Arquitectonica-OneDrive-Linux.md`)

## Mejores Prácticas para Desarrollo Seguro de Componentes de Sistema

---

## 1. Introducción y Problemática

LNXDrive involucra tres categorías de componentes que interactúan profundamente con el sistema operativo:

| Componente | Riesgo si falla | Impacto potencial |
|------------|-----------------|-------------------|
| **Servicios systemd** | Medio-Alto | Procesos zombie, recursos no liberados, conflictos de puertos |
| **FUSE filesystem** | Alto | Kernel panic (raro), procesos colgados en I/O, puntos de montaje huérfanos |
| **Extensiones desktop** | Alto | GNOME Shell crash, Nautilus inutilizable, sesión gráfica corrupta |

**El desafío**: Desarrollar y depurar estos componentes sin:
- Bloquear la sesión de trabajo del desarrollador
- Corromper el entorno de escritorio
- Dejar el sistema en estado inconsistente
- Perder trabajo por crashes del entorno gráfico

---

## 2. Estrategias de Aislamiento

### 2.1 Niveles de Aislamiento Disponibles

```
┌─────────────────────────────────────────────────────────────────────────┐
│  ESPECTRO DE AISLAMIENTO                                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Menor aislamiento ◄─────────────────────────────────► Mayor aislamiento│
│  Mayor velocidad                                        Menor velocidad │
│                                                                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │ Proceso │ │Namespace│ │Container│ │   VM    │ │ VM con  │           │
│  │ normal  │ │ aislado │ │(Podman) │ │ headless│ │ GUI     │           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
│       │           │           │           │           │                 │
│       ▼           ▼           ▼           ▼           ▼                 │
│   Unit tests   FUSE dev   systemd    FUSE+systemd  Desktop              │
│   Mocks        aislado    testing    integration   extensions           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Recomendación por Componente

| Componente | Desarrollo | Testing Unitario | Testing Integración | Testing E2E |
|------------|------------|------------------|---------------------|-------------|
| Core (lógica) | Host directo | Host directo | Host directo | Container |
| FUSE | Namespace/Container | Container | VM headless | VM headless |
| systemd service | User systemd | Container con systemd | VM headless | VM headless |
| Nautilus extension | Container GUI | Container GUI | VM con GNOME | VM con GNOME |
| GNOME Shell ext | VM con GNOME | VM con GNOME | VM con GNOME | VM con GNOME |

---

## 3. Testing de Servicios systemd

### 3.1 User-Mode systemd (Desarrollo Local Seguro)

**systemd --user** permite ejecutar servicios en el espacio del usuario sin privilegios root:

```bash
# Estructura de directorios para desarrollo
~/.config/systemd/user/
└── lnxdrive-dev.service

# Archivo de servicio para desarrollo
# ~/.config/systemd/user/lnxdrive-dev.service
[Unit]
Description=LNXDrive Development Daemon
After=default.target

[Service]
Type=simple
ExecStart=/home/dev/proyectos/lnxdrive/target/debug/lnxdrive-daemon --config ~/.config/lnxdrive-dev/config.yaml
ExecStop=/bin/kill -SIGTERM $MAINPID
Restart=no
Environment=RUST_LOG=debug
Environment=LNXDRIVE_DEV=1
StandardOutput=journal
StandardError=journal

# Aislamiento adicional incluso en modo usuario
PrivateTmp=true
NoNewPrivileges=true

[Install]
WantedBy=default.target
```

```bash
# Comandos de desarrollo
systemctl --user daemon-reload
systemctl --user start lnxdrive-dev
systemctl --user status lnxdrive-dev
journalctl --user -u lnxdrive-dev -f  # Logs en tiempo real

# Reinicio rápido durante desarrollo
systemctl --user restart lnxdrive-dev
```

**Ventajas:**
- No requiere sudo
- Aislado del systemd del sistema
- Fácil de limpiar (solo borrar el archivo .service)
- Logs separados en el journal del usuario

**Limitaciones:**
- No puede usar puertos < 1024
- No tiene acceso a /run/systemd/system
- Algunas directivas de sandboxing no funcionan

### 3.2 Contenedor con systemd Completo

Para testing más realista, usar **Podman** (no Docker) porque soporta systemd nativamente:

```dockerfile
# Containerfile.systemd-test
FROM fedora:41

# Instalar systemd y dependencias
RUN dnf install -y systemd systemd-libs dbus-daemon \
    fuse3 fuse3-libs sqlite \
    && dnf clean all

# Habilitar systemd como init
RUN systemctl mask systemd-remount-fs.service \
    && systemctl mask systemd-firstboot.service

# Copiar servicio de LNXDrive
COPY lnxdrive-daemon /usr/local/bin/
COPY lnxdrive.service /etc/systemd/system/

# Habilitar servicio
RUN systemctl enable lnxdrive.service

# Punto de entrada es systemd
CMD ["/sbin/init"]
```

```bash
# Ejecutar contenedor con systemd
podman run -d --name lnxdrive-test \
    --privileged \
    --systemd=always \
    -v ./config:/etc/lnxdrive:ro \
    -v ./test-data:/home/test/OneDrive \
    localhost/lnxdrive-systemd-test

# Interactuar con el servicio dentro del contenedor
podman exec lnxdrive-test systemctl status lnxdrive
podman exec lnxdrive-test journalctl -u lnxdrive -f

# Simular reinicio del servicio
podman exec lnxdrive-test systemctl restart lnxdrive

# Simular crash y recovery
podman exec lnxdrive-test kill -9 $(pgrep lnxdrive-daemon)
# systemd debería reiniciarlo automáticamente
```

### 3.3 Framework de Testing para systemd

```rust
// tests/systemd_integration.rs
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

/// Trait para abstraer operaciones systemd (mockeable)
#[async_trait]
pub trait SystemdController {
    async fn start_service(&self, name: &str) -> Result<(), SystemdError>;
    async fn stop_service(&self, name: &str) -> Result<(), SystemdError>;
    async fn get_status(&self, name: &str) -> Result<ServiceStatus, SystemdError>;
    async fn wait_for_status(&self, name: &str, status: ServiceStatus, timeout: Duration) -> Result<(), SystemdError>;
}

/// Implementación real para testing de integración
pub struct RealSystemdController {
    user_mode: bool,
}

impl RealSystemdController {
    pub fn user_mode() -> Self {
        Self { user_mode: true }
    }

    fn systemctl(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("systemctl");
        if self.user_mode {
            cmd.arg("--user");
        }
        cmd.args(args);
        cmd
    }
}

/// Mock para unit tests
pub struct MockSystemdController {
    pub services: HashMap<String, ServiceStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_starts_and_responds_to_dbus() {
        let ctl = RealSystemdController::user_mode();

        // Iniciar servicio
        ctl.start_service("lnxdrive-dev").await.unwrap();

        // Esperar a que esté activo
        ctl.wait_for_status("lnxdrive-dev", ServiceStatus::Active, Duration::from_secs(10))
            .await
            .unwrap();

        // Verificar que responde a DBus
        let conn = zbus::Connection::session().await.unwrap();
        let proxy = LNXDriveProxy::new(&conn).await.unwrap();
        let status = proxy.get_status().await.unwrap();

        assert_eq!(status, "idle");

        // Cleanup
        ctl.stop_service("lnxdrive-dev").await.unwrap();
    }

    #[tokio::test]
    async fn test_service_recovers_from_crash() {
        let ctl = RealSystemdController::user_mode();
        ctl.start_service("lnxdrive-dev").await.unwrap();

        // Obtener PID
        let pid = ctl.get_main_pid("lnxdrive-dev").await.unwrap();

        // Simular crash
        Command::new("kill").args(["-9", &pid.to_string()]).output().unwrap();

        // systemd debería reiniciarlo (si Restart=always)
        sleep(Duration::from_secs(2)).await;

        let status = ctl.get_status("lnxdrive-dev").await.unwrap();
        assert_eq!(status, ServiceStatus::Active);

        // Cleanup
        ctl.stop_service("lnxdrive-dev").await.unwrap();
    }
}
```

---

## 4. Testing de FUSE Filesystem

### 4.1 El Problema del FUSE Colgado

FUSE es particularmente peligroso porque:
- Un proceso FUSE que crashea deja el punto de montaje en estado "Transport endpoint is not connected"
- Procesos que acceden al mountpoint quedan en estado D (uninterruptible sleep)
- `umount` normal falla; requiere `fusermount -uz` (lazy unmount)
- En el peor caso, requiere reiniciar para limpiar

### 4.2 Desarrollo FUSE en Namespace Aislado

Usar **unshare** para crear un namespace de montaje separado:

```bash
#!/bin/bash
# scripts/dev-fuse-isolated.sh
# Ejecuta el daemon FUSE en un namespace de montaje aislado

set -e

# Crear directorio temporal para el mount
MOUNT_DIR=$(mktemp -d /tmp/lnxdrive-fuse-dev.XXXXXX)
DATA_DIR=$(mktemp -d /tmp/lnxdrive-data-dev.XXXXXX)

cleanup() {
    echo "Limpiando..."
    # Intentar unmount limpio
    fusermount -uz "$MOUNT_DIR" 2>/dev/null || true
    rmdir "$MOUNT_DIR" 2>/dev/null || true
    rm -rf "$DATA_DIR" 2>/dev/null || true
    echo "Limpieza completada"
}
trap cleanup EXIT

echo "Mount point: $MOUNT_DIR"
echo "Data dir: $DATA_DIR"

# Ejecutar en namespace aislado
# --mount: namespace de montajes separado
# --map-root-user: mapear usuario actual a root dentro del namespace
unshare --mount --map-root-user -- bash -c "
    # Dentro del namespace, los mounts son privados
    mount --make-rprivate /

    # Ejecutar daemon FUSE
    RUST_LOG=debug cargo run --bin lnxdrive-fuse -- \
        --mount-point '$MOUNT_DIR' \
        --data-dir '$DATA_DIR' \
        --foreground
"
```

### 4.3 Contenedor para FUSE

```dockerfile
# Containerfile.fuse-test
FROM rust:1.83-slim

RUN apt-get update && apt-get install -y \
    fuse3 libfuse3-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Permitir FUSE sin ser root
RUN echo "user_allow_other" >> /etc/fuse.conf

WORKDIR /app
COPY . .

RUN cargo build --release --bin lnxdrive-fuse

# Usuario no-root para desarrollo
RUN useradd -m devuser
USER devuser

CMD ["./target/release/lnxdrive-fuse", "--foreground"]
```

```bash
# Ejecutar contenedor con FUSE
podman run -it --rm \
    --device /dev/fuse \
    --cap-add SYS_ADMIN \
    --security-opt apparmor=unconfined \
    -v ./test-data:/data \
    -v ./mount:/mnt/lnxdrive \
    localhost/lnxdrive-fuse-test
```

### 4.4 Testing de FUSE con libfuse Mock

Para unit tests, mockear las operaciones FUSE sin montar realmente:

```rust
// src/fuse/mod.rs

/// Trait que abstrae operaciones del filesystem
/// Permite testing sin FUSE real
#[async_trait]
pub trait FileSystemOps: Send + Sync {
    async fn getattr(&self, path: &Path) -> Result<FileAttr, FsError>;
    async fn readdir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>;
    async fn read(&self, path: &Path, offset: u64, size: u32) -> Result<Vec<u8>, FsError>;
    async fn write(&self, path: &Path, offset: u64, data: &[u8]) -> Result<u32, FsError>;
    async fn open(&self, path: &Path, flags: OpenFlags) -> Result<FileHandle, FsError>;
    // ... más operaciones
}

/// Implementación real que usa el backend de LNXDrive
pub struct LNXDriveFs {
    core: Arc<LNXDriveCore>,
    state: Arc<StateRepository>,
}

/// Implementación mock para testing
#[cfg(test)]
pub struct MockFs {
    pub files: HashMap<PathBuf, MockFile>,
    pub call_log: Arc<Mutex<Vec<FsCall>>>,
}

#[cfg(test)]
impl MockFs {
    pub fn with_files(files: Vec<(&str, &[u8])>) -> Self {
        let mut map = HashMap::new();
        for (path, content) in files {
            map.insert(PathBuf::from(path), MockFile {
                content: content.to_vec(),
                attr: default_attr(),
            });
        }
        Self { files: map, call_log: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn assert_called(&self, call: FsCall) {
        let log = self.call_log.lock().unwrap();
        assert!(log.contains(&call), "Expected call {:?} not found in {:?}", call, *log);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_triggers_hydration_for_placeholder() {
        let mock = MockFs::with_files(vec![
            ("/document.pdf", b""),  // Placeholder vacío
        ]);

        // Marcar como placeholder
        mock.files.get_mut(&PathBuf::from("/document.pdf"))
            .unwrap()
            .attr.set_xattr("user.lnxdrive.state", "online");

        let fs = LNXDriveFsWrapper::new(mock.clone());

        // Intentar leer debería triggear hidratación
        let result = fs.read(Path::new("/document.pdf"), 0, 1024).await;

        // Verificar que se llamó a hydrate
        mock.assert_called(FsCall::Hydrate("/document.pdf".into()));
    }
}
```

### 4.5 Script de Recuperación de FUSE

```bash
#!/bin/bash
# scripts/fuse-recovery.sh
# Recupera de un FUSE colgado sin reiniciar

MOUNT_POINT="${1:-/home/$USER/LNXDrive}"

echo "Intentando recuperar punto de montaje: $MOUNT_POINT"

# 1. Encontrar procesos atascados en el mountpoint
echo "Procesos usando el mountpoint:"
lsof +D "$MOUNT_POINT" 2>/dev/null || echo "  (ninguno o no accesible)"

# 2. Intentar lazy unmount
echo "Intentando lazy unmount..."
fusermount -uz "$MOUNT_POINT" 2>/dev/null && echo "  Éxito" || echo "  Falló"

# 3. Si aún está montado, forzar
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "Aún montado, intentando umount -l..."
    sudo umount -l "$MOUNT_POINT" 2>/dev/null || true
fi

# 4. Matar procesos FUSE huérfanos
echo "Buscando procesos lnxdrive-fuse huérfanos..."
pkill -9 -f "lnxdrive-fuse" 2>/dev/null || true

# 5. Verificar estado final
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "ADVERTENCIA: El mountpoint sigue activo. Puede requerir reinicio."
else
    echo "Mountpoint limpio."
fi
```

---

## 5. Testing de Extensiones de Desktop

### 5.1 El Problema de las Extensiones de Desktop

Las extensiones de Nautilus y GNOME Shell son especialmente peligrosas:

| Tipo | Riesgo | Síntoma de fallo |
|------|--------|------------------|
| Nautilus extension | Alto | Nautilus crashea, no se puede navegar archivos |
| GNOME Shell extension | Crítico | Shell crashea, sesión gráfica inutilizable |
| DBus service | Medio | Servicios no responden, timeout en operaciones |

**Regla de oro**: NUNCA desarrollar extensiones de desktop en la sesión principal de trabajo.

### 5.2 Estrategia: Sesión Anidada con GNOME Nested

GNOME puede ejecutarse dentro de sí mismo usando Wayland nested:

```bash
#!/bin/bash
# scripts/dev-gnome-nested.sh
# Ejecuta una sesión GNOME anidada para desarrollo de extensiones

# Crear directorio de configuración aislado
export XDG_CONFIG_HOME="/tmp/lnxdrive-gnome-dev/config"
export XDG_DATA_HOME="/tmp/lnxdrive-gnome-dev/data"
export XDG_CACHE_HOME="/tmp/lnxdrive-gnome-dev/cache"
export XDG_RUNTIME_DIR="/tmp/lnxdrive-gnome-dev/runtime"

mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

# Copiar extensión en desarrollo
mkdir -p "$XDG_DATA_HOME/gnome-shell/extensions"
cp -r ./lnxdrive-gnome-extension "$XDG_DATA_HOME/gnome-shell/extensions/lnxdrive@enigmora.org"

# Copiar extensión de Nautilus
mkdir -p "$XDG_DATA_HOME/nautilus-python/extensions"
cp ./lnxdrive-nautilus.py "$XDG_DATA_HOME/nautilus-python/extensions/"

# Ejecutar GNOME Shell anidado
# Esto abre una ventana con un GNOME Shell completo dentro
dbus-run-session -- gnome-shell --nested --wayland

# Cuando cierres la ventana, todo se limpia automáticamente
echo "Sesión anidada terminada. Limpiando..."
rm -rf /tmp/lnxdrive-gnome-dev
```

**Ventajas:**
- Si la extensión crashea, solo afecta la ventana anidada
- La sesión principal sigue funcionando
- Fácil de reiniciar (cerrar y abrir ventana)
- Configuración aislada

**Limitaciones:**
- Rendimiento reducido
- Algunas features de Wayland no funcionan igual
- No prueba integración real con el sistema

### 5.3 Máquina Virtual con GNOME Completo

Para testing E2E, usar una VM con GNOME real:

```bash
# Crear VM con GNOME para testing
# Usando libvirt/virt-manager

# Opción 1: Imagen cloud de Fedora
wget https://download.fedoraproject.org/pub/fedora/linux/releases/41/Cloud/x86_64/images/Fedora-Cloud-Base-41-1.4.x86_64.qcow2

# Crear VM
virt-install \
    --name lnxdrive-test-gnome \
    --memory 4096 \
    --vcpus 2 \
    --disk path=./lnxdrive-test.qcow2,size=20 \
    --cdrom Fedora-Workstation-Live-x86_64-41-1.4.iso \
    --os-variant fedora41 \
    --graphics spice \
    --video qxl
```

**Script de provisioning para la VM:**

```bash
#!/bin/bash
# scripts/vm-provision.sh
# Ejecutar dentro de la VM de testing

# Instalar dependencias
sudo dnf install -y \
    gnome-shell \
    nautilus \
    nautilus-python \
    fuse3 \
    dbus-daemon \
    rsync

# Crear usuario de prueba
sudo useradd -m testuser
sudo passwd testuser  # Establecer contraseña

# Configurar autologin para testing automatizado
sudo mkdir -p /etc/gdm
sudo tee /etc/gdm/custom.conf << 'EOF'
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=testuser
EOF

# Crear directorio compartido para código
sudo mkdir -p /mnt/lnxdrive-dev
sudo chown testuser:testuser /mnt/lnxdrive-dev
```

### 5.4 Testing Automatizado de Extensiones Nautilus

```python
# tests/nautilus_extension_test.py
"""
Tests de integración para la extensión de Nautilus.
Requiere ejecutarse en un entorno con GNOME (VM o nested session).
"""

import subprocess
import time
import gi
gi.require_version('Nautilus', '4.0')
gi.require_version('Gtk', '4.0')
from gi.repository import Nautilus, Gtk, Gio, GLib

class TestNautilusExtension:
    """Tests para la extensión de Nautilus de LNXDrive."""

    @classmethod
    def setup_class(cls):
        """Preparar entorno de testing."""
        # Verificar que estamos en un entorno gráfico
        assert 'DISPLAY' in os.environ or 'WAYLAND_DISPLAY' in os.environ

        # Iniciar Nautilus en modo headless para testing
        cls.nautilus_proc = subprocess.Popen(
            ['nautilus', '--new-window', '/tmp/lnxdrive-test'],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        time.sleep(2)  # Esperar a que inicie

    @classmethod
    def teardown_class(cls):
        """Limpiar después de tests."""
        cls.nautilus_proc.terminate()
        cls.nautilus_proc.wait()

    def test_extension_loads_without_crash(self):
        """Verificar que la extensión se carga sin crashear Nautilus."""
        # Si llegamos aquí, Nautilus no crasheó al cargar
        assert self.nautilus_proc.poll() is None, "Nautilus crasheó"

    def test_overlay_icons_appear(self):
        """Verificar que los iconos de overlay aparecen en archivos sincronizados."""
        # Crear archivo de prueba con xattr de LNXDrive
        test_file = Path('/tmp/lnxdrive-test/synced-file.txt')
        test_file.write_text('test content')
        os.setxattr(str(test_file), 'user.lnxdrive.state', b'hydrated')

        # Usar D-Bus para verificar que Nautilus ve el emblema
        # (implementación específica según la extensión)

    def test_context_menu_appears(self):
        """Verificar que el menú contextual de LNXDrive aparece."""
        # Usar Dogtail o similar para testing de UI
        pass
```

### 5.5 Contenedor con Escritorio para CI

Para CI/CD, usar contenedores con un servidor X virtual:

```dockerfile
# Containerfile.desktop-test
FROM fedora:41

# Instalar GNOME y Xvfb
RUN dnf install -y \
    gnome-shell \
    nautilus \
    nautilus-python \
    xorg-x11-server-Xvfb \
    dbus-x11 \
    at-spi2-core \
    python3-dogtail \
    && dnf clean all

# Script de entrada que inicia Xvfb y GNOME
COPY docker-entrypoint.sh /
RUN chmod +x /docker-entrypoint.sh

# Variables de entorno para display virtual
ENV DISPLAY=:99
ENV DBUS_SESSION_BUS_ADDRESS=unix:path=/run/dbus/system_bus_socket

ENTRYPOINT ["/docker-entrypoint.sh"]
```

```bash
#!/bin/bash
# docker-entrypoint.sh

# Iniciar Xvfb (X virtual framebuffer)
Xvfb :99 -screen 0 1920x1080x24 &
sleep 1

# Iniciar dbus session
eval $(dbus-launch --sh-syntax)

# Iniciar GNOME Shell en modo headless
gnome-shell --wayland --headless &
sleep 3

# Ejecutar tests
exec "$@"
```

---

## 6. Mocking de APIs Externas

### 6.1 Mock de Microsoft Graph API

Para no depender de la API real durante desarrollo:

```rust
// src/adapters/graph/mock.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

/// Servidor mock de Microsoft Graph para testing
pub struct MockGraphServer {
    server: MockServer,
}

impl MockGraphServer {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Configura respuesta para delta API
    pub async fn mock_delta_response(&self, items: Vec<DriveItem>, delta_token: &str) {
        let response = serde_json::json!({
            "value": items,
            "@odata.deltaLink": format!("{}?token={}", self.url(), delta_token)
        });

        Mock::given(method("GET"))
            .and(path("/me/drive/root/delta"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&self.server)
            .await;
    }

    /// Configura respuesta de throttling (429)
    pub async fn mock_throttling(&self, retry_after: u32) {
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", retry_after.to_string())
            )
            .expect(1)  // Solo una vez, luego responde normal
            .mount(&self.server)
            .await;
    }

    /// Configura upload session
    pub async fn mock_upload_session(&self, upload_url: &str) {
        Mock::given(method("POST"))
            .and(path_regex(r"/me/drive/items/.*/createUploadSession"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uploadUrl": upload_url,
                "expirationDateTime": "2026-01-30T00:00:00Z"
            })))
            .mount(&self.server)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delta_sync_with_mock() {
        let mock = MockGraphServer::start().await;

        // Configurar respuesta mock
        mock.mock_delta_response(vec![
            DriveItem { id: "1".into(), name: "file1.txt".into(), .. },
            DriveItem { id: "2".into(), name: "file2.txt".into(), .. },
        ], "token123").await;

        // Crear cliente apuntando al mock
        let client = GraphClient::new(&mock.url(), "test-token");

        // Ejecutar sync
        let result = client.get_delta(None).await.unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.delta_token, Some("token123".into()));
    }

    #[tokio::test]
    async fn test_throttling_retry() {
        let mock = MockGraphServer::start().await;

        // Primera llamada: throttling
        mock.mock_throttling(5).await;

        // Segunda llamada: éxito
        mock.mock_delta_response(vec![], "token").await;

        let client = GraphClient::new(&mock.url(), "test-token");

        // El cliente debería reintentar automáticamente
        let result = client.get_delta(None).await.unwrap();

        // Verificar que funcionó después del retry
        assert!(result.items.is_empty());
    }
}
```

### 6.2 Mock de Sistema de Archivos Local

```rust
// src/adapters/filesystem/mock.rs

use tempfile::TempDir;

/// Filesystem temporal para testing
pub struct TestFileSystem {
    root: TempDir,
}

impl TestFileSystem {
    pub fn new() -> Self {
        Self { root: TempDir::new().unwrap() }
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// Crea estructura de archivos para testing
    pub fn setup_files(&self, files: &[(&str, &[u8])]) {
        for (path, content) in files {
            let full_path = self.root.path().join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(full_path, content).unwrap();
        }
    }

    /// Crea placeholder con xattrs
    pub fn setup_placeholder(&self, path: &str, remote_id: &str, size: u64) {
        let full_path = self.root.path().join(path);
        std::fs::write(&full_path, "").unwrap();  // Archivo vacío

        xattr::set(&full_path, "user.lnxdrive.state", b"online").unwrap();
        xattr::set(&full_path, "user.lnxdrive.remote_id", remote_id.as_bytes()).unwrap();
        xattr::set(&full_path, "user.lnxdrive.size", size.to_string().as_bytes()).unwrap();
    }

    /// Verifica estado de archivo
    pub fn assert_file_state(&self, path: &str, expected_state: &str) {
        let full_path = self.root.path().join(path);
        let state = xattr::get(&full_path, "user.lnxdrive.state")
            .unwrap()
            .unwrap();
        assert_eq!(String::from_utf8(state).unwrap(), expected_state);
    }
}
```

---

## 7. Entorno de Desarrollo Integrado

### 7.1 Estructura de Proyecto para Testing

```
lnxdrive/
├── crates/
│   ├── lnxdrive-core/           # Lógica de negocio (testeable directamente)
│   ├── lnxdrive-fuse/           # FUSE filesystem
│   ├── lnxdrive-graph/          # Cliente MS Graph
│   └── lnxdrive-audit/          # Sistema de auditoría
├── apps/
│   ├── lnxdrive-daemon/         # Daemon principal
│   ├── lnxdrive-cli/            # CLI
│   └── lnxdrive-gnome/          # Extensiones GNOME
├── tests/
│   ├── unit/                    # Tests unitarios (cargo test)
│   ├── integration/             # Tests de integración (requieren setup)
│   └── e2e/                     # Tests end-to-end (requieren VM)
├── scripts/
│   ├── dev-fuse-isolated.sh     # Desarrollo FUSE aislado
│   ├── dev-gnome-nested.sh      # Desarrollo GNOME nested
│   ├── vm-provision.sh          # Provisioning de VM
│   └── fuse-recovery.sh         # Recuperación de FUSE
├── docker/
│   ├── Containerfile.systemd    # Container con systemd
│   ├── Containerfile.fuse       # Container para FUSE
│   └── Containerfile.desktop    # Container con desktop
├── .github/
│   └── workflows/
│       ├── unit-tests.yml       # CI: tests unitarios
│       ├── integration.yml      # CI: tests de integración
│       └── e2e.yml              # CI: tests E2E (con VM)
└── Makefile                     # Comandos de desarrollo
```

### 7.2 Makefile para Desarrollo

```makefile
# Makefile

.PHONY: test test-unit test-integration test-e2e dev-fuse dev-gnome vm-create vm-test

# --- Testing ---

test: test-unit test-integration

test-unit:
	cargo test --workspace

test-integration:
	./scripts/run-integration-tests.sh

test-e2e:
	./scripts/run-e2e-tests.sh

# --- Desarrollo Local ---

dev-daemon:
	RUST_LOG=debug cargo run --bin lnxdrive-daemon -- --config ./dev-config.yaml

dev-fuse:
	./scripts/dev-fuse-isolated.sh

dev-gnome:
	./scripts/dev-gnome-nested.sh

dev-cli:
	cargo run --bin lnxdrive -- $(ARGS)

# --- Servicios systemd (modo usuario) ---

service-install:
	cp ./systemd/lnxdrive-dev.service ~/.config/systemd/user/
	systemctl --user daemon-reload

service-start:
	systemctl --user start lnxdrive-dev

service-stop:
	systemctl --user stop lnxdrive-dev

service-logs:
	journalctl --user -u lnxdrive-dev -f

# --- Contenedores ---

container-build-all:
	podman build -f docker/Containerfile.systemd -t lnxdrive-systemd .
	podman build -f docker/Containerfile.fuse -t lnxdrive-fuse .
	podman build -f docker/Containerfile.desktop -t lnxdrive-desktop .

container-test-systemd:
	podman run --rm --privileged --systemd=always lnxdrive-systemd

container-test-fuse:
	podman run --rm --device /dev/fuse --cap-add SYS_ADMIN lnxdrive-fuse

# --- VM para E2E ---

vm-create:
	./scripts/vm-create.sh

vm-start:
	virsh start lnxdrive-test-gnome

vm-ssh:
	ssh testuser@lnxdrive-test-vm

vm-sync-code:
	rsync -avz --exclude target/ ./ testuser@lnxdrive-test-vm:/mnt/lnxdrive-dev/

vm-test:
	ssh testuser@lnxdrive-test-vm "cd /mnt/lnxdrive-dev && make test-e2e"

# --- Limpieza ---

clean:
	cargo clean
	rm -rf /tmp/lnxdrive-*

clean-fuse:
	./scripts/fuse-recovery.sh

clean-containers:
	podman rm -f $$(podman ps -aq --filter ancestor=lnxdrive-*) 2>/dev/null || true
```

### 7.3 Configuración de VS Code para Desarrollo

```json
// .vscode/launch.json
{
    "version": "0.2.0",
    "configurations": [
        {
            "name": "Debug Daemon",
            "type": "lldb",
            "request": "launch",
            "program": "${workspaceFolder}/target/debug/lnxdrive-daemon",
            "args": ["--config", "${workspaceFolder}/dev-config.yaml"],
            "env": {
                "RUST_LOG": "debug",
                "LNXDRIVE_DEV": "1"
            },
            "cwd": "${workspaceFolder}"
        },
        {
            "name": "Debug CLI Command",
            "type": "lldb",
            "request": "launch",
            "program": "${workspaceFolder}/target/debug/lnxdrive",
            "args": ["status", "--json"],
            "env": {
                "RUST_LOG": "debug"
            }
        },
        {
            "name": "Debug FUSE (Isolated)",
            "type": "lldb",
            "request": "launch",
            "program": "/usr/bin/unshare",
            "args": [
                "--mount", "--map-root-user", "--",
                "${workspaceFolder}/target/debug/lnxdrive-fuse",
                "--mount-point", "/tmp/lnxdrive-fuse-debug",
                "--foreground"
            ],
            "env": {
                "RUST_LOG": "debug"
            }
        },
        {
            "name": "Run Unit Tests",
            "type": "lldb",
            "request": "launch",
            "cargo": {
                "args": ["test", "--workspace", "--no-run"],
                "filter": {
                    "kind": "test"
                }
            }
        }
    ]
}
```

```json
// .vscode/tasks.json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "Start Dev Daemon (systemd user)",
            "type": "shell",
            "command": "make service-start",
            "problemMatcher": []
        },
        {
            "label": "View Daemon Logs",
            "type": "shell",
            "command": "make service-logs",
            "isBackground": true,
            "problemMatcher": []
        },
        {
            "label": "Start GNOME Nested Session",
            "type": "shell",
            "command": "make dev-gnome",
            "problemMatcher": []
        },
        {
            "label": "Run Integration Tests",
            "type": "shell",
            "command": "make test-integration",
            "group": "test",
            "problemMatcher": []
        }
    ]
}
```

---

## 8. Pipeline de CI/CD

### 8.1 GitHub Actions para Tests Multinivel

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  # --- Tests Unitarios (rápidos, sin aislamiento especial) ---
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run unit tests
        run: cargo test --workspace --lib --bins

  # --- Tests de Integración (con contenedores) ---
  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    services:
      mock-graph:
        image: ghcr.io/enigmora/lnxdrive-mock-graph:latest
        ports:
          - 8080:8080

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install FUSE
        run: sudo apt-get install -y fuse3 libfuse3-dev

      - name: Run integration tests
        run: cargo test --workspace --test '*'
        env:
          GRAPH_MOCK_URL: http://localhost:8080
          RUST_LOG: debug

  # --- Tests de systemd (con contenedor systemd) ---
  systemd-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v4

      - name: Build systemd container
        run: |
          podman build -f docker/Containerfile.systemd -t lnxdrive-systemd .

      - name: Run systemd tests
        run: |
          podman run --rm --privileged --systemd=always \
            -v ./tests/systemd:/tests:ro \
            lnxdrive-systemd \
            /tests/run-systemd-tests.sh

  # --- Tests E2E (con VM, solo en merge a main) ---
  e2e-tests:
    runs-on: [self-hosted, linux, vm-capable]
    needs: [integration-tests, systemd-tests]
    if: github.ref == 'refs/heads/main'

    steps:
      - uses: actions/checkout@v4

      - name: Start test VM
        run: |
          virsh start lnxdrive-test-gnome || true
          sleep 30  # Esperar boot

      - name: Sync code to VM
        run: make vm-sync-code

      - name: Run E2E tests in VM
        run: make vm-test

      - name: Collect test artifacts
        if: always()
        run: |
          scp testuser@lnxdrive-test-vm:/tmp/test-results/* ./test-results/

      - name: Upload test results
        uses: actions/upload-artifact@v4
        with:
          name: e2e-test-results
          path: test-results/
```

---

## 9. Logging, Tracing y Obtención de Registros para Depuración

### 9.1 El Desafío del Logging en Sistemas Distribuidos

LNXDrive es un sistema con múltiples componentes que se ejecutan en diferentes contextos:

```
┌─────────────────────────────────────────────────────────────────────────┐
│  COMPONENTES Y SUS CONTEXTOS DE LOGGING                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐               │
│  │  lnxdrive   │     │  lnxdrive   │     │  lnxdrive   │               │
│  │   daemon    │     │    fuse     │     │    cli      │               │
│  │             │     │             │     │             │               │
│  │ systemd     │     │ proceso     │     │ proceso     │               │
│  │ journal     │     │ standalone  │     │ efímero     │               │
│  └──────┬──────┘     └──────┬──────┘     └──────┬──────┘               │
│         │                   │                   │                       │
│         └───────────────────┼───────────────────┘                       │
│                             │                                           │
│                    ┌────────▼────────┐                                  │
│                    │ Correlación de  │                                  │
│                    │     eventos     │                                  │
│                    └────────┬────────┘                                  │
│                             │                                           │
│  ┌─────────────┐     ┌──────▼──────┐     ┌─────────────┐               │
│  │  Nautilus   │     │   GNOME     │     │   DBus      │               │
│  │  Extension  │     │   Shell     │     │  Messages   │               │
│  │  (Python)   │     │  Extension  │     │             │               │
│  │             │     │  (JS)       │     │             │               │
│  │ stdout/     │     │ journalctl  │     │ dbus-monitor│               │
│  │ syslog      │     │ --user      │     │             │               │
│  └─────────────┘     └─────────────┘     └─────────────┘               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

**Problemas a resolver:**
1. Cada componente tiene su propio mecanismo de logging
2. Los logs no están correlacionados
3. En caso de crash, los logs pueden perderse
4. Debugging de componentes en containers/VMs requiere acceso remoto

### 9.2 Estrategia de Logging Unificada

#### 9.2.1 Formato de Log Estructurado

Todos los componentes Rust deben usar logging estructurado con `tracing`:

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"
tracing-journald = "0.3"  # Para integración con systemd journal

// src/logging.rs
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging(config: &LogConfig) -> Result<LogGuard, LogError> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Capa para journald (producción)
    let journald_layer = tracing_journald::layer()
        .ok()
        .filter(|_| config.use_journald);

    // Capa para archivo JSON (debugging detallado)
    let (file_writer, file_guard) = if let Some(ref path) = config.file_path {
        let file_appender = tracing_appender::rolling::daily(path, "lnxdrive.log");
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        (Some(writer), Some(guard))
    } else {
        (None, None)
    };

    let json_layer = file_writer.map(|w| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(w)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
    });

    // Capa para terminal (desarrollo)
    let terminal_layer = if config.use_terminal {
        Some(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true))
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(journald_layer)
        .with(json_layer)
        .with(terminal_layer)
        .init();

    Ok(LogGuard { _file_guard: file_guard })
}
```

#### 9.2.2 Contexto de Correlación

Usar **trace IDs** para correlacionar eventos entre componentes:

```rust
use uuid::Uuid;

/// Contexto de operación que se propaga entre componentes
#[derive(Clone, Debug)]
pub struct OperationContext {
    /// ID único de la operación (se propaga via DBus, headers, etc.)
    pub trace_id: Uuid,
    /// ID del span actual
    pub span_id: Uuid,
    /// Cuenta/namespace activo
    pub account_id: String,
    /// Archivo siendo procesado (si aplica)
    pub file_path: Option<PathBuf>,
}

impl OperationContext {
    pub fn new(account_id: &str) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            account_id: account_id.to_string(),
            file_path: None,
        }
    }

    /// Crear span de tracing con contexto
    pub fn span(&self, name: &str) -> tracing::Span {
        tracing::info_span!(
            "op",
            trace_id = %self.trace_id,
            span_id = %self.span_id,
            account = %self.account_id,
            file = ?self.file_path,
            op = name
        )
    }
}

// Uso en código
async fn sync_file(&self, ctx: &OperationContext, path: &Path) -> Result<()> {
    let span = ctx.span("sync_file").entered();

    tracing::info!(path = ?path, "Starting file sync");

    // La operación...
    let result = self.upload(path).await;

    match &result {
        Ok(_) => tracing::info!("File sync completed successfully"),
        Err(e) => tracing::error!(error = ?e, "File sync failed"),
    }

    result
}
```

### 9.3 Logging por Componente

#### 9.3.1 Daemon (systemd service)

```bash
# Ver logs en tiempo real
journalctl --user -u lnxdrive -f

# Ver logs con nivel debug
journalctl --user -u lnxdrive -p debug

# Filtrar por trace_id específico
journalctl --user -u lnxdrive -o json | jq 'select(.TRACE_ID == "abc-123")'

# Ver logs de las últimas 2 horas
journalctl --user -u lnxdrive --since "2 hours ago"

# Exportar logs para análisis
journalctl --user -u lnxdrive -o json > lnxdrive-logs.json
```

#### 9.3.2 FUSE Filesystem

```bash
# El script de desarrollo ya configura logging
./scripts/dev-fuse-isolated.sh

# Los logs van a:
# - Terminal (RUST_LOG=debug)
# - Archivo: /tmp/lnxdrive-fuse-dev.XXXXXX/fuse.log

# Ver logs de FUSE en tiempo real
tail -f /tmp/lnxdrive-fuse-dev.*/fuse.log | jq .

# Filtrar operaciones de un archivo específico
tail -f /tmp/lnxdrive-fuse-dev.*/fuse.log | jq 'select(.path | contains("documento.pdf"))'
```

#### 9.3.3 Extensión de Nautilus (Python)

```python
# lnxdrive-nautilus.py
import logging
import sys
from gi.repository import GLib

# Configurar logging para extensión de Nautilus
def setup_logging():
    # Crear logger específico para LNXDrive
    logger = logging.getLogger('lnxdrive-nautilus')
    logger.setLevel(logging.DEBUG)

    # Handler para syslog (capturado por journald)
    from logging.handlers import SysLogHandler
    syslog_handler = SysLogHandler(address='/dev/log')
    syslog_handler.setFormatter(logging.Formatter(
        'lnxdrive-nautilus[%(process)d]: %(levelname)s %(message)s'
    ))
    logger.addHandler(syslog_handler)

    # Handler para archivo (debugging)
    file_handler = logging.FileHandler(
        GLib.get_user_cache_dir() + '/lnxdrive/nautilus-extension.log'
    )
    file_handler.setFormatter(logging.Formatter(
        '%(asctime)s [%(levelname)s] %(funcName)s: %(message)s'
    ))
    logger.addHandler(file_handler)

    return logger

logger = setup_logging()

class LNXDriveExtension(GObject.GObject, Nautilus.MenuProvider):
    def __init__(self):
        logger.info("LNXDrive Nautilus extension loaded")

    def get_file_items(self, files):
        logger.debug(f"get_file_items called with {len(files)} files")
        # ...
```

**Ver logs de extensión Nautilus:**

```bash
# Logs van a journald via syslog
journalctl -f | grep lnxdrive-nautilus

# O al archivo de cache
tail -f ~/.cache/lnxdrive/nautilus-extension.log

# Reiniciar Nautilus para recargar extensión y ver logs de inicio
nautilus -q && nautilus &
journalctl -f | grep -E "(nautilus|lnxdrive)"
```

#### 9.3.4 Extensión de GNOME Shell (JavaScript)

```javascript
// extension.js
const ExtensionUtils = imports.misc.extensionUtils;
const Me = ExtensionUtils.getCurrentExtension();

// Logger personalizado para GNOME Shell
class LNXDriveLogger {
    constructor() {
        this.prefix = '[LNXDrive]';
    }

    _format(level, message, data) {
        const timestamp = new Date().toISOString();
        const dataStr = data ? ` ${JSON.stringify(data)}` : '';
        return `${timestamp} ${this.prefix} ${level}: ${message}${dataStr}`;
    }

    debug(message, data) {
        if (Me.metadata['debug-mode']) {
            log(this._format('DEBUG', message, data));
        }
    }

    info(message, data) {
        log(this._format('INFO', message, data));
    }

    warn(message, data) {
        log(this._format('WARN', message, data));
    }

    error(message, error) {
        logError(error, this._format('ERROR', message));
    }
}

const logger = new LNXDriveLogger();

// Uso
function init() {
    logger.info('Extension initializing', { version: Me.metadata.version });
}

function enable() {
    logger.info('Extension enabled');
    // ...
}
```

**Ver logs de GNOME Shell:**

```bash
# Logs de GNOME Shell van a journald
journalctl -f /usr/bin/gnome-shell

# Filtrar solo logs de LNXDrive
journalctl -f /usr/bin/gnome-shell | grep "\[LNXDrive\]"

# En sesión nested, los logs van a la terminal donde se lanzó
dbus-run-session -- gnome-shell --nested --wayland 2>&1 | grep "\[LNXDrive\]"

# Looking Glass (debugger interactivo de GNOME Shell)
# Presionar Alt+F2, escribir "lg", Enter
# En la pestaña "Extensions" se pueden ver errores
```

---

## 10. Automatización de Depuración para Desarrollo Asistido

### 10.1 Filosofía: Debugging como Comando Simple

La depuración en LNXDrive está diseñada para ser **invocable programáticamente** durante el ciclo de codificación, no solo en CI/CD. Esto permite que:

1. **Desarrolladores** ejecuten pruebas y obtengan diagnósticos con un solo comando
2. **Agentes de IA** (Claude, Copilot, etc.) puedan probar cambios, obtener logs y analizar resultados
3. **Scripts de automatización** validen el código antes de commits

```
┌─────────────────────────────────────────────────────────────────────────┐
│  FLUJO DE DEPURACIÓN AUTOMATIZADA                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   Desarrollador                    Agente de IA                         │
│        │                                │                               │
│        ▼                                ▼                               │
│   ┌─────────────────────────────────────────────────────────┐           │
│   │           INTERFAZ UNIFICADA DE COMANDOS                │           │
│   │                                                         │           │
│   │  make test-unit        → Ejecutar tests unitarios       │           │
│   │  make test-component X → Probar componente específico   │           │
│   │  make diagnose         → Obtener estado y logs          │           │
│   │  make validate         → Validación completa rápida     │           │
│   │                                                         │           │
│   └─────────────────────────────────────────────────────────┘           │
│                              │                                          │
│                              ▼                                          │
│   ┌─────────────────────────────────────────────────────────┐           │
│   │              SALIDA ESTRUCTURADA (JSON)                 │           │
│   │                                                         │           │
│   │  {                                                      │           │
│   │    "success": false,                                    │           │
│   │    "tests_run": 45,                                     │           │
│   │    "tests_failed": 2,                                   │           │
│   │    "failures": [...],                                   │           │
│   │    "logs": "base64...",                                 │           │
│   │    "suggestions": ["Check rate limiter config..."]      │           │
│   │  }                                                      │           │
│   │                                                         │           │
│   │  Parseabe por humanos Y por agentes de IA               │           │
│   └─────────────────────────────────────────────────────────┘           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 10.2 Comandos de Depuración Unificados

```makefile
# Makefile - Comandos de depuración automatizada

OUTPUT ?= human  # human | json

# Validación rápida (< 30 segundos)
.PHONY: validate
validate:
	@./scripts/dev-validate.sh --output $(OUTPUT)

# Tests por componente
.PHONY: test-unit test-core test-fuse test-graph test-integration

test-unit:
	@./scripts/dev-test.sh unit --output $(OUTPUT)

test-core:
	@./scripts/dev-test.sh component core --output $(OUTPUT)

test-fuse:
	@./scripts/dev-test.sh component fuse --output $(OUTPUT)

test-graph:
	@./scripts/dev-test.sh component graph --output $(OUTPUT)

test-integration:
	@./scripts/dev-test.sh integration --output $(OUTPUT)

# Diagnóstico del sistema
.PHONY: diagnose diagnose-quick

diagnose:
	@./scripts/dev-diagnose.sh --output $(OUTPUT)

diagnose-quick:
	@./scripts/dev-diagnose.sh --quick --output $(OUTPUT)
```

### 10.3 Interfaz para Agentes de IA

```bash
# Comandos que un agente de IA puede ejecutar durante codificación

# 1. Validación rápida después de cambiar código (< 30s)
make validate OUTPUT=json

# 2. Probar solo el componente modificado
make test-core OUTPUT=json
make test-fuse OUTPUT=json
make test-graph OUTPUT=json

# 3. Obtener diagnóstico si algo no funciona
make diagnose OUTPUT=json

# 4. Obtener solo el estado rápido (sin logs históricos)
make diagnose-quick OUTPUT=json

# 5. Analizar logs recientes buscando errores específicos
make analyze-logs OUTPUT=json FILTER="error|panic|failed"

# 6. Ejecutar tests de integración (más lento, ~2-5 min)
make test-integration OUTPUT=json
```

---

## 11. Resumen de Mejores Prácticas

### 11.1 Matriz de Decisiones

| Escenario | Herramienta Recomendada | Razón |
|-----------|------------------------|-------|
| Desarrollo de lógica de negocio | Host directo + cargo test | Rápido, sin overhead |
| Desarrollo de FUSE | Namespace aislado (unshare) | Protege sistema host |
| Testing de FUSE | Container con --device /dev/fuse | Reproducible, aislado |
| Desarrollo de systemd service | systemd --user | Sin privilegios, fácil cleanup |
| Testing de systemd | Container con Podman --systemd=always | systemd real en entorno controlado |
| Desarrollo de extensión Nautilus | GNOME nested session | No afecta sesión principal |
| Desarrollo de GNOME Shell ext | VM con GNOME | Aislamiento total |
| Testing E2E de desktop | VM con GNOME + automatización | Entorno real completo |
| CI para tests unitarios | GitHub Actions ubuntu-latest | Rápido, económico |
| CI para tests integración | Container en CI | Controlado, reproducible |
| CI para tests E2E | Self-hosted runner con VM | Necesario para GUI |

### 11.2 Reglas de Oro

1. **Nunca desarrollar extensiones de desktop en la sesión principal**
2. **Siempre tener un script de recuperación de FUSE a mano**
3. **Usar systemd --user para desarrollo local de servicios**
4. **Mockear APIs externas para tests unitarios e integración**
5. **Reservar tests E2E con VM para CI, no desarrollo diario**
6. **Mantener tiempos de feedback cortos: unit < 1min, integration < 5min**

### 11.3 Checklist Pre-Commit

```bash
#!/bin/bash
# scripts/pre-commit-check.sh

set -e

echo "=== Running pre-commit checks ==="

# 1. Formateo
echo "Checking formatting..."
cargo fmt --check

# 2. Linting
echo "Running clippy..."
cargo clippy --workspace -- -D warnings

# 3. Tests unitarios
echo "Running unit tests..."
cargo test --workspace --lib

# 4. Tests de integración rápidos
echo "Running quick integration tests..."
cargo test --workspace --test 'quick_*'

echo "=== All checks passed ==="
```

---

## 12. Conclusiones

El desarrollo de LNXDrive requiere un enfoque por capas para testing:

1. **Capa 1 (más frecuente)**: Tests unitarios en host, sin aislamiento especial
2. **Capa 2**: Tests de integración en containers/namespaces
3. **Capa 3**: Tests de systemd en containers con systemd
4. **Capa 4 (menos frecuente)**: Tests E2E en VM con desktop completo

La inversión en infraestructura de testing (scripts, containers, VM) se paga rápidamente al evitar:
- Sesiones gráficas corruptas
- Puntos de montaje FUSE huérfanos
- Servicios systemd mal configurados en el sistema real
- Pérdida de trabajo por crashes del entorno

---

*Documento de investigación generado el 29 de enero de 2026*
*Proyecto Enigmora - LNXDrive*

---

## Ver también

- [Estrategia de Testing](../06-Testing/01-estrategia-testing.md) - Resumen de la estrategia
- [Testing de FUSE](../06-Testing/03-testing-fuse.md) - Testing específico de FUSE
- [Testing de systemd](../06-Testing/02-testing-systemd.md) - Testing de servicios
- [Testing de Desktop](../06-Testing/04-testing-desktop.md) - Testing de extensiones
- [CI/CD Pipeline](../06-Testing/06-ci-cd-pipeline.md) - Automatización en CI
- [Logging y Tracing](../06-Testing/07-logging-tracing.md) - Sistema de logs
