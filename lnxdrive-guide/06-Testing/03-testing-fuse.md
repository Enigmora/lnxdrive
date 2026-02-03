# Testing de FUSE Filesystem

> **Ubicación:** `06-Testing/03-testing-fuse.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

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
    // ... mas operaciones
}

/// Implementacion real que usa el backend de LNXDrive
pub struct LNXDriveFs {
    core: Arc<LNXDriveCore>,
    state: Arc<StateRepository>,
}

/// Implementacion mock para testing
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
            ("/document.pdf", b""),  // Placeholder vacio
        ]);

        // Marcar como placeholder
        mock.files.get_mut(&PathBuf::from("/document.pdf"))
            .unwrap()
            .attr.set_xattr("user.lnxdrive.state", "online");

        let fs = LNXDriveFsWrapper::new(mock.clone());

        // Intentar leer deberia triggear hidratacion
        let result = fs.read(Path::new("/document.pdf"), 0, 1024).await;

        // Verificar que se llamo a hydrate
        mock.assert_called(FsCall::Hydrate("/document.pdf".into()));
    }
}
```

### 4.5 Script de Recuperacion de FUSE

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
fusermount -uz "$MOUNT_POINT" 2>/dev/null && echo "  Exito" || echo "  Fallo"

# 3. Si aun esta montado, forzar
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "Aun montado, intentando umount -l..."
    sudo umount -l "$MOUNT_POINT" 2>/dev/null || true
fi

# 4. Matar procesos FUSE huerfanos
echo "Buscando procesos lnxdrive-fuse huerfanos..."
pkill -9 -f "lnxdrive-fuse" 2>/dev/null || true

# 5. Verificar estado final
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "ADVERTENCIA: El mountpoint sigue activo. Puede requerir reinicio."
else
    echo "Mountpoint limpio."
fi
```

---

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de Servicios systemd](02-testing-systemd.md) - Testing de servicios systemd
- [Mocking de APIs Externas](05-mocking-apis.md) - Mock del sistema de archivos local
- [Logging y Tracing](07-logging-tracing.md) - Logging especial para FUSE
