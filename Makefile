# LNXDrive Makefile
#
# Build and test targets for development and CI.
# Container targets use Podman for systemd-based functional testing.

.PHONY: build test-unit \
        container-build-systemd container-test-daemon \
        container-shell container-stop container-clean

# Stub crates excluded from compilation (not yet implemented)
EXCLUDE_STUBS = --exclude lnxdrive-fuse \
                --exclude lnxdrive-conflict \
                --exclude lnxdrive-audit \
                --exclude lnxdrive-telemetry

CONTAINER_IMAGE  = localhost/lnxdrive-systemd
CONTAINER_NAME   = lnxdrive-test

# ==============================================================================
# Build
# ==============================================================================

build:
	cargo build --release --workspace $(EXCLUDE_STUBS)

# ==============================================================================
# Unit Tests
# ==============================================================================

test-unit:
	cargo test --workspace $(EXCLUDE_STUBS)

# ==============================================================================
# Container: Build
# ==============================================================================

container-build-systemd: build
	podman build \
		-f docker/Containerfile.systemd \
		-t $(CONTAINER_IMAGE) \
		.

# ==============================================================================
# Container: Run Functional Tests
# ==============================================================================

container-test-daemon: container-stop
	@echo "--- Starting systemd container ---"
	podman run -d \
		--name $(CONTAINER_NAME) \
		--systemd=always \
		$(CONTAINER_IMAGE)
	@echo "--- Waiting for systemd boot ---"
	sleep 5
	@echo "--- Running functional tests ---"
	podman exec \
		--user testuser \
		--env XDG_RUNTIME_DIR=/run/user/$$(podman exec $(CONTAINER_NAME) id -u testuser) \
		--env DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$$(podman exec $(CONTAINER_NAME) id -u testuser)/bus \
		$(CONTAINER_NAME) \
		/usr/local/bin/test-daemon-functional.sh
	@echo "--- Cleaning up container ---"
	podman stop $(CONTAINER_NAME) >/dev/null 2>&1 || true
	podman rm $(CONTAINER_NAME) >/dev/null 2>&1 || true

# ==============================================================================
# Container: Interactive Shell
# ==============================================================================

container-shell:
	@if ! podman ps --format '{{.Names}}' | grep -q '^$(CONTAINER_NAME)$$'; then \
		echo "--- Starting systemd container ---"; \
		podman run -d \
			--name $(CONTAINER_NAME) \
			--systemd=always \
			$(CONTAINER_IMAGE); \
		sleep 5; \
	fi
	podman exec -it \
		--user testuser \
		--env XDG_RUNTIME_DIR=/run/user/$$(podman exec $(CONTAINER_NAME) id -u testuser) \
		--env DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$$(podman exec $(CONTAINER_NAME) id -u testuser)/bus \
		$(CONTAINER_NAME) \
		/bin/bash

# ==============================================================================
# Container: Cleanup
# ==============================================================================

container-stop:
	@podman stop $(CONTAINER_NAME) >/dev/null 2>&1 || true
	@podman rm $(CONTAINER_NAME) >/dev/null 2>&1 || true

container-clean: container-stop
	@podman rmi $(CONTAINER_IMAGE) >/dev/null 2>&1 || true
	@echo "Container image and artifacts removed."
