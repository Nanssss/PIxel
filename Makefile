# Load environment variables
-include .env
export

BINARY_NAME = PIxel

.PHONY: all host target clean

# Default action builds for the cross-target (ARMv6)
all: target

# Build your local host machine
host:
	cargo build --release

# Build for Raspberry Pi Zero
target:
	cross build --target arm-unknown-linux-gnueabihf --release

# Build and copy the compiled binary to the Raspberry Pi Zero
deploy: target
	@echo "==> Deploying binary to $(TARGET_HOST):$(TARGET_DIR)..."
	ssh $(TARGET_HOST) "mkdir -p $(TARGET_DIR)"
	scp target/arm-unknown-linux-gnueabihf/release/$(BINARY_NAME) $(TARGET_HOST):$(TARGET_DIR)/
	@echo "==> Deployment complete!"

# Run the binary on the remote Pi (runs in the background using nohup)
run:
	@echo "==> Starting $(BINARY_NAME) on $(TARGET_HOST)..."
	@ssh $(TARGET_HOST) "cd $(TARGET_DIR) && sudo sh -c 'setsid nohup ./$(BINARY_NAME) &'"

# Stop any running instance of the binary on the remote Pi
stop:
	@echo "==> Stopping $(BINARY_NAME) on $(TARGET_HOST)..."
	@ssh $(TARGET_HOST) "sudo pkill -x $(BINARY_NAME) || true"
	@echo "==> Application stopped."

# Convenient shortcut to restart (stop -> deploy -> run)
restart: stop deploy run

logs:
	@ssh $(TARGET_HOST) "cd $(TARGET_DIR)/log && tail -n 100 -f \`ls -t * | head -n 1\`"

# Clean both Cargo and the external C library
clean:
	@if [ -d "deps/rpi-rgb-led-matrix" ]; then \
		$(MAKE) -C deps/rpi-rgb-led-matrix clean; \
	fi
	cargo clean
