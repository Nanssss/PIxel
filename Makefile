# Load environment variables
-include .env
export

BINARY_NAME = PIxel
# Name of the systemd service on the Pi (see deploy/pixel.service)
SERVICE = pixel

.PHONY: all host sim target deploy install run stop restart logs clean

# Default action builds for the cross-target (ARMv6)
all: target

# Build your local host machine
host:
	cargo build --release

# Run on your local host machine, with the matrix simulated in a window
sim:
	@mkdir -p target/debug && ln -sfn ../../in target/debug/in
	cargo run --features simulator

# Build for Raspberry Pi Zero
target:
	cross build --target arm-unknown-linux-gnueabihf --release

# Build and copy the compiled binary to the Raspberry Pi Zero
deploy: target
	@echo "==> Deploying binary to $(TARGET_HOST):$(TARGET_DIR)..."
	ssh $(TARGET_HOST) "mkdir -p $(TARGET_DIR)"
	scp -r in/ target/arm-unknown-linux-gnueabihf/release/$(BINARY_NAME) $(TARGET_HOST):$(TARGET_DIR)/
	@echo "==> Deployment complete!"

# Install the systemd service on the Pi (once): PIxel then starts at boot, and restarts after a crash.
# The absolute deploy directory is asked to the Pi (TARGET_DIR may be relative to the home directory),
# then filled into the unit file, which is written to /etc/systemd/system.
install:
	@echo "==> Installing the $(SERVICE) service on $(TARGET_HOST)..."
	@dir=$$(ssh $(TARGET_HOST) "cd $(TARGET_DIR) && pwd") && \
	sed -e "s|@TARGET_DIR@|$$dir|g" -e "s|@BINARY_NAME@|$(BINARY_NAME)|g" deploy/$(SERVICE).service | \
	ssh $(TARGET_HOST) "sudo tee /etc/systemd/system/$(SERVICE).service > /dev/null && \
		sudo systemctl daemon-reload && sudo systemctl enable $(SERVICE)"
	@echo "==> Service installed and enabled at boot."

# Start the service on the remote Pi
run:
	@echo "==> Starting $(BINARY_NAME) on $(TARGET_HOST)..."
	@ssh $(TARGET_HOST) "sudo systemctl start $(SERVICE)"

# Stop the service on the remote Pi (SIGTERM: the app turns the panel off before exiting)
stop:
	@echo "==> Stopping $(BINARY_NAME) on $(TARGET_HOST)..."
	@ssh $(TARGET_HOST) "sudo systemctl stop $(SERVICE)"
	@echo "==> Application stopped."

# Convenient shortcut to restart (stop -> deploy -> run).
# Stopping first is required: a running binary can't be overwritten ("Text file busy").
restart: stop deploy run

logs:
	@ssh $(TARGET_HOST) "cd $(TARGET_DIR)/log && tail -n 100 -f \`ls -t * | head -n 1\`"

# Clean both Cargo and the external C library
clean:
	@if [ -d "deps/rpi-rgb-led-matrix" ]; then \
		$(MAKE) -C deps/rpi-rgb-led-matrix clean; \
	fi
	cargo clean
