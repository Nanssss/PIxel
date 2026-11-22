# 👾 PIxel

A LED Matrix Display using a **Raspberry Pi**, **Rust**, and the awesome [hzeller rpi-rgb-led-matrix](https://github.com/hzeller/rpi-rgb-led-matrix) library.

*See future improvements and notes in [wiki](https://github.com/Nanssss/PIxel/wiki).*

⏳ This project is still in early-stage development ⏳

# Setup

## Get the sources

The C library is a git submodule: clone with it, or fetch it afterwards.

```sh
git clone --recurse-submodules https://github.com/Nanssss/PIxel.git
# or, in an existing clone:
git submodule update --init
```

## Tools needed

- **Rust** (with `cargo`), through [rustup](https://rustup.rs/).
- **[cross](https://github.com/cross-rs/cross)** and **Docker**, to build for the Pi Zero (ARMv6): `cargo install cross`.
  The build image adds `libclang` for bindgen (see `Dockerfile.cross`).
- **SDL2**, only for the simulator: `sudo apt install libsdl2-dev`.

## Tune your config as you want

Edit `in/config.toml` as you want. Here, you can enable/disable each state, and set the place of the weather forecast.

## Create your Google OAuth2 credentials

If you want to use the calendar feature, you must start by getting Google OAuth2 credentials.

First, go to the [Google Cloud console](https://console.cloud.google.com/). Head to APIs and Services -> Credentials -> Create credentials -> OAuth client ID. The recommended type is "Desktop app": it is the one meant for devices that can't open a browser themselves.

Then, once your credentials are created, download them and put them in `in/credentials.json`.

On the first run, the app logs a URL to open in a browser to grant access (on the Pi: `journalctl -u pixel`). The tokens are then cached in `in/tokencache.json`.

# Build and run

## On your PC, with the simulator

```sh
make sim
```

Runs the app in a window that shows the panel (`cargo run --features simulator`): no Raspberry Pi needed.

## On the Raspberry Pi

Create a `.env` file at the root of the project (it is git-ignored), telling where to deploy:

```sh
TARGET_HOST=pi@raspberrypi.local   # SSH destination of the Pi
TARGET_DIR=pixel                   # deploy directory on the Pi (relative to the home directory, or absolute)
```

Then:

| Command        | What it does                                                                 |
|----------------|------------------------------------------------------------------------------|
| `make target`  | Builds the release binary for the Pi Zero (`arm-unknown-linux-gnueabihf`)   |
| `make deploy`  | Builds, then copies the binary and `in/` (config, credentials) to the Pi      |
| `make install` | Installs the systemd service (once): the app then starts at boot             |
| `make run` / `make stop` | Starts / stops the service                                         |
| `make restart` | Stops the service, deploys, and starts it again                              |
| `make logs`    | Follows the latest log file on the Pi                                        |
| `make clean`   | Cleans the Rust build and the C library                                      |

The first time: `make deploy`, then `make install`, then `make run`.

The app needs root to drive the GPIO: the service starts it as root, and it drops its privileges right after creating the matrix.

## Before committing

Format the code with `cargo fmt` (rules in `rustfmt.toml`).

# License

This project is an open-source software licensed under the **GNU General Public License v2.0**.

It is designed to interface with the [hzeller/rpi-rgb-led-matrix](https://github.com/hzeller/rpi-rgb-led-matrix) library, which is also licensed under the GNU GPL v2.0. 

See the [LICENSE](LICENSE) file for the full text.

