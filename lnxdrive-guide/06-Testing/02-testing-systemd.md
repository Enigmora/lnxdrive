# Testing de Servicios systemd

> **Ubicación:** `06-Testing/02-testing-systemd.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

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

# Reinicio rapido durante desarrollo
systemctl --user restart lnxdrive-dev
```

**Ventajas:**
- No requiere sudo
- Aislado del systemd del sistema
- Facil de limpiar (solo borrar el archivo .service)
- Logs separados en el journal del usuario

**Limitaciones:**
- No puede usar puertos < 1024
- No tiene acceso a /run/systemd/system
- Algunas directivas de sandboxing no funcionan

### 3.2 Contenedor con systemd Completo

Para testing mas realista, usar **Podman** (no Docker) porque soporta systemd nativamente:

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
# systemd deberia reiniciarlo automaticamente
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

/// Implementacion real para testing de integracion
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

        // Esperar a que este activo
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

        // systemd deberia reiniciarlo (si Restart=always)
        sleep(Duration::from_secs(2)).await;

        let status = ctl.get_status("lnxdrive-dev").await.unwrap();
        assert_eq!(status, ServiceStatus::Active);

        // Cleanup
        ctl.stop_service("lnxdrive-dev").await.unwrap();
    }
}
```

---

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de FUSE Filesystem](03-testing-fuse.md) - Testing del sistema de archivos FUSE
- [Pipeline CI/CD](06-ci-cd-pipeline.md) - Tests de systemd en CI
- [Logging y Tracing](07-logging-tracing.md) - Logs del daemon con journald
