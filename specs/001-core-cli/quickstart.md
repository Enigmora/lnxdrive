# Quickstart: Core + CLI (Fase 1)

**Branch**: `001-core-cli` | **Date**: 2026-02-03

## Prerequisites

Before starting development:

1. **Rust toolchain**: Install via rustup
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup default stable  # 1.75+
   ```

2. **System dependencies**:
   ```bash
   # Fedora/RHEL
   sudo dnf install sqlite-devel dbus-devel libsecret-devel

   # Ubuntu/Debian
   sudo apt install libsqlite3-dev libdbus-1-dev libsecret-1-dev

   # Arch
   sudo pacman -S sqlite dbus libsecret
   ```

3. **Development tools**:
   ```bash
   cargo install cargo-watch cargo-audit sqlx-cli
   ```

4. **Azure App Registration** (for OAuth2):
   - Go to [Azure Portal](https://portal.azure.com)
   - Register new app with redirect URI: `http://127.0.0.1:8400/callback`
   - Add delegated permissions: `Files.ReadWrite`, `offline_access`, `User.Read`
   - Note the Application (client) ID

## Project Setup

1. **Clone and setup workspace**:
   ```bash
   cd /path/to/lnxdrive
   git checkout 001-core-cli
   ```

2. **Create workspace structure**:
   ```bash
   mkdir -p crates/{lnxdrive-core,lnxdrive-graph,lnxdrive-sync,lnxdrive-cache,lnxdrive-ipc,lnxdrive-cli,lnxdrive-daemon}/src
   mkdir -p tests/{integration,e2e}
   mkdir -p config
   ```

3. **Initialize Cargo workspace** (`Cargo.toml` at root):
   ```toml
   [workspace]
   resolver = "2"
   members = [
       "crates/lnxdrive-core",
       "crates/lnxdrive-graph",
       "crates/lnxdrive-sync",
       "crates/lnxdrive-cache",
       "crates/lnxdrive-ipc",
       "crates/lnxdrive-cli",
       "crates/lnxdrive-daemon",
   ]

   [workspace.package]
   version = "0.1.0"
   edition = "2021"
   rust-version = "1.75"
   license = "GPL-3.0-or-later"
   repository = "https://github.com/Enigmora/lnxdrive"

   [workspace.dependencies]
   tokio = { version = "1.35", features = ["full"] }
   reqwest = { version = "0.11", features = ["json", "rustls-tls"], default-features = false }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   thiserror = "1.0"
   anyhow = "1.0"
   tracing = "0.1"
   tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   ```

## Development Workflow

### Running the CLI (development)

```bash
# Build and run
cargo run -p lnxdrive-cli -- auth login

# Watch mode (auto-rebuild on changes)
cargo watch -x "run -p lnxdrive-cli -- status"
```

### Running tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p lnxdrive-core

# With logging
RUST_LOG=debug cargo test -- --nocapture

# Integration tests only
cargo test --test '*' -p lnxdrive-cache
```

### Database migrations

```bash
# Create new migration
sqlx migrate add -r create_accounts_table

# Run migrations (dev)
DATABASE_URL="sqlite:~/.local/share/lnxdrive/state.db" sqlx migrate run

# Prepare offline mode (for CI)
cargo sqlx prepare --workspace
```

### Security audit

```bash
cargo audit
```

## Configuration

### Default config location

```
~/.config/lnxdrive/config.yaml
```

### Minimal config for development

```yaml
sync:
  root: ~/OneDrive
  poll_interval: 30

auth:
  app_id: "your-azure-app-id-here"

logging:
  level: debug
```

### Data directories

```
~/.local/share/lnxdrive/
├── state.db        # SQLite database
├── lnxdrive.log    # Application log
└── uploads/        # Temp upload session data
```

## First Run Checklist

1. [ ] Build succeeds: `cargo build`
2. [ ] Tests pass: `cargo test`
3. [ ] Auth works: `cargo run -p lnxdrive-cli -- auth login`
4. [ ] Status shows: `cargo run -p lnxdrive-cli -- auth status`
5. [ ] Sync runs: `cargo run -p lnxdrive-cli -- sync --dry-run`

## Troubleshooting

### "Token storage failed"

Ensure libsecret is running:
```bash
# Check if secret service is available
dbus-send --session --print-reply \
  --dest=org.freedesktop.secrets \
  /org/freedesktop/secrets \
  org.freedesktop.DBus.Introspectable.Introspect
```

### "Database migration failed"

```bash
# Reset database (development only!)
rm ~/.local/share/lnxdrive/state.db
cargo run -p lnxdrive-cli -- sync
```

### "D-Bus connection failed"

```bash
# Check session bus
echo $DBUS_SESSION_BUS_ADDRESS

# If empty, ensure you're in a desktop session or:
eval $(dbus-launch --sh-syntax)
```

## Constitution Compliance Reminders

Before submitting PR:

- [ ] Hexagonal architecture maintained (domain has no external deps)
- [ ] Newtypes used for domain types (SyncPath, FileHash, etc.)
- [ ] thiserror for library errors, anyhow for app errors
- [ ] Unit tests for core (80% coverage target)
- [ ] No secrets in logs (check all tracing statements)
- [ ] AILOG created for changes >10 lines in business logic
- [ ] Conventional commit message format
