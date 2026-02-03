# Testing de Extensiones de Desktop

> **Ubicación:** `06-Testing/04-testing-desktop.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

---

## 5. Testing de Extensiones de Desktop

### 5.1 El Problema de las Extensiones de Desktop

Las extensiones de Nautilus y GNOME Shell son especialmente peligrosas:

| Tipo | Riesgo | Sintoma de fallo |
|------|--------|------------------|
| Nautilus extension | Alto | Nautilus crashea, no se puede navegar archivos |
| GNOME Shell extension | Critico | Shell crashea, sesion grafica inutilizable |
| DBus service | Medio | Servicios no responden, timeout en operaciones |

**Regla de oro**: NUNCA desarrollar extensiones de desktop en la sesion principal de trabajo.

### 5.2 Estrategia: Sesion Anidada con GNOME Nested

GNOME puede ejecutarse dentro de si mismo usando Wayland nested:

```bash
#!/bin/bash
# scripts/dev-gnome-nested.sh
# Ejecuta una sesion GNOME anidada para desarrollo de extensiones

# Crear directorio de configuracion aislado
export XDG_CONFIG_HOME="/tmp/lnxdrive-gnome-dev/config"
export XDG_DATA_HOME="/tmp/lnxdrive-gnome-dev/data"
export XDG_CACHE_HOME="/tmp/lnxdrive-gnome-dev/cache"
export XDG_RUNTIME_DIR="/tmp/lnxdrive-gnome-dev/runtime"

mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

# Copiar extension en desarrollo
mkdir -p "$XDG_DATA_HOME/gnome-shell/extensions"
cp -r ./lnxdrive-gnome-extension "$XDG_DATA_HOME/gnome-shell/extensions/lnxdrive@enigmora.org"

# Copiar extension de Nautilus
mkdir -p "$XDG_DATA_HOME/nautilus-python/extensions"
cp ./lnxdrive-nautilus.py "$XDG_DATA_HOME/nautilus-python/extensions/"

# Ejecutar GNOME Shell anidado
# Esto abre una ventana con un GNOME Shell completo dentro
dbus-run-session -- gnome-shell --nested --wayland

# Cuando cierres la ventana, todo se limpia automaticamente
echo "Sesion anidada terminada. Limpiando..."
rm -rf /tmp/lnxdrive-gnome-dev
```

**Ventajas:**
- Si la extension crashea, solo afecta la ventana anidada
- La sesion principal sigue funcionando
- Facil de reiniciar (cerrar y abrir ventana)
- Configuracion aislada

**Limitaciones:**
- Rendimiento reducido
- Algunas features de Wayland no funcionan igual
- No prueba integracion real con el sistema

### 5.3 Maquina Virtual con GNOME Completo

Para testing E2E, usar una VM con GNOME real:

```bash
# Crear VM con GNOME para testing
# Usando libvirt/virt-manager

# Opcion 1: Imagen cloud de Fedora
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
sudo passwd testuser  # Establecer contrasena

# Configurar autologin para testing automatizado
sudo mkdir -p /etc/gdm
sudo tee /etc/gdm/custom.conf << 'EOF'
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=testuser
EOF

# Crear directorio compartido para codigo
sudo mkdir -p /mnt/lnxdrive-dev
sudo chown testuser:testuser /mnt/lnxdrive-dev
```

### 5.4 Testing Automatizado de Extensiones Nautilus

```python
# tests/nautilus_extension_test.py
"""
Tests de integracion para la extension de Nautilus.
Requiere ejecutarse en un entorno con GNOME (VM o nested session).
"""

import subprocess
import time
import gi
gi.require_version('Nautilus', '4.0')
gi.require_version('Gtk', '4.0')
from gi.repository import Nautilus, Gtk, Gio, GLib

class TestNautilusExtension:
    """Tests para la extension de Nautilus de LNXDrive."""

    @classmethod
    def setup_class(cls):
        """Preparar entorno de testing."""
        # Verificar que estamos en un entorno grafico
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
        """Limpiar despues de tests."""
        cls.nautilus_proc.terminate()
        cls.nautilus_proc.wait()

    def test_extension_loads_without_crash(self):
        """Verificar que la extension se carga sin crashear Nautilus."""
        # Si llegamos aqui, Nautilus no crasheo al cargar
        assert self.nautilus_proc.poll() is None, "Nautilus crasheo"

    def test_overlay_icons_appear(self):
        """Verificar que los iconos de overlay aparecen en archivos sincronizados."""
        # Crear archivo de prueba con xattr de LNXDrive
        test_file = Path('/tmp/lnxdrive-test/synced-file.txt')
        test_file.write_text('test content')
        os.setxattr(str(test_file), 'user.lnxdrive.state', b'hydrated')

        # Usar D-Bus para verificar que Nautilus ve el emblema
        # (implementacion especifica segun la extension)

    def test_context_menu_appears(self):
        """Verificar que el menu contextual de LNXDrive aparece."""
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

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de Servicios systemd](02-testing-systemd.md) - Testing de servicios systemd
- [Pipeline CI/CD](06-ci-cd-pipeline.md) - Tests E2E con VM en CI
- [Logging y Tracing](07-logging-tracing.md) - Logs de extensiones Nautilus y GNOME Shell
