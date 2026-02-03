# Pipeline de CI/CD

> **Ubicación:** `06-Testing/06-ci-cd-pipeline.md`
> **Relacionado:** [Estrategia de Testing](01-estrategia-testing.md), [Anexo A: Metodologías de Testing](../Anexos/A-metodologias-testing-completo.md)

---

## 7. Entorno de Desarrollo Integrado

### 7.1 Estructura de Proyecto para Testing

```
lnxdrive/
├── crates/
│   ├── lnxdrive-core/           # Logica de negocio (testeable directamente)
│   ├── lnxdrive-fuse/           # FUSE filesystem
│   ├── lnxdrive-graph/          # Cliente MS Graph
│   └── lnxdrive-audit/          # Sistema de auditoria
├── apps/
│   ├── lnxdrive-daemon/         # Daemon principal
│   ├── lnxdrive-cli/            # CLI
│   └── lnxdrive-gnome/          # Extensiones GNOME
├── tests/
│   ├── unit/                    # Tests unitarios (cargo test)
│   ├── integration/             # Tests de integracion (requieren setup)
│   └── e2e/                     # Tests end-to-end (requieren VM)
├── scripts/
│   ├── dev-fuse-isolated.sh     # Desarrollo FUSE aislado
│   ├── dev-gnome-nested.sh      # Desarrollo GNOME nested
│   ├── vm-provision.sh          # Provisioning de VM
│   └── fuse-recovery.sh         # Recuperacion de FUSE
├── docker/
│   ├── Containerfile.systemd    # Container con systemd
│   ├── Containerfile.fuse       # Container para FUSE
│   └── Containerfile.desktop    # Container con desktop
├── .github/
│   └── workflows/
│       ├── unit-tests.yml       # CI: tests unitarios
│       ├── integration.yml      # CI: tests de integracion
│       └── e2e.yml              # CI: tests E2E (con VM)
└── Makefile                     # Comandos de desarrollo
```

### 7.2 Makefile para Desarrollo

```makefile
# Makefile

.PHONY: test test-unit test-integration test-e2e dev-fuse dev-gnome vm-create vm-test

# --- Testing -------------------------------------------------------------------

test: test-unit test-integration

test-unit:
	cargo test --workspace

test-integration:
	./scripts/run-integration-tests.sh

test-e2e:
	./scripts/run-e2e-tests.sh

# --- Desarrollo Local ----------------------------------------------------------

dev-daemon:
	RUST_LOG=debug cargo run --bin lnxdrive-daemon -- --config ./dev-config.yaml

dev-fuse:
	./scripts/dev-fuse-isolated.sh

dev-gnome:
	./scripts/dev-gnome-nested.sh

dev-cli:
	cargo run --bin lnxdrive -- $(ARGS)

# --- Servicios systemd (modo usuario) ------------------------------------------

service-install:
	cp ./systemd/lnxdrive-dev.service ~/.config/systemd/user/
	systemctl --user daemon-reload

service-start:
	systemctl --user start lnxdrive-dev

service-stop:
	systemctl --user stop lnxdrive-dev

service-logs:
	journalctl --user -u lnxdrive-dev -f

# --- Contenedores --------------------------------------------------------------

container-build-all:
	podman build -f docker/Containerfile.systemd -t lnxdrive-systemd .
	podman build -f docker/Containerfile.fuse -t lnxdrive-fuse .
	podman build -f docker/Containerfile.desktop -t lnxdrive-desktop .

container-test-systemd:
	podman run --rm --privileged --systemd=always lnxdrive-systemd

container-test-fuse:
	podman run --rm --device /dev/fuse --cap-add SYS_ADMIN lnxdrive-fuse

# --- VM para E2E ---------------------------------------------------------------

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

# --- Limpieza ------------------------------------------------------------------

clean:
	cargo clean
	rm -rf /tmp/lnxdrive-*

clean-fuse:
	./scripts/fuse-recovery.sh

clean-containers:
	podman rm -f $$(podman ps -aq --filter ancestor=lnxdrive-*) 2>/dev/null || true
```

### 7.3 Configuracion de VS Code para Desarrollo

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
  # --- Tests Unitarios (rapidos, sin aislamiento especial) ---------------------
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

  # --- Tests de Integracion (con contenedores) ---------------------------------
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

  # --- Tests de systemd (con contenedor systemd) -------------------------------
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

  # --- Tests E2E (con VM, solo en merge a main) --------------------------------
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

## Ver tambien

- [Estrategia de Testing](01-estrategia-testing.md) - Introduccion y estrategias de aislamiento
- [Testing de Servicios systemd](02-testing-systemd.md) - Testing de servicios systemd
- [Mocking de APIs Externas](05-mocking-apis.md) - Servicios mock para CI
- [Automatizacion de Depuracion](08-automatizacion-depuracion.md) - Comandos automatizados
