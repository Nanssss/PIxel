# 👾 PIxel

A LED Matrix Display using a **Raspberry Pi**, **Rust**, and the awesome [hzeller rpi-rgb-led-matrix](https://github.com/hzeller/rpi-rgb-led-matrix) library.

*See future improvements and notes in [wiki](https://github.com/Nanssss/PIxel/wiki).*

⏳ This project is still in early-stage development ⏳

# Setup


## Create your Google OAuth2 credentials

If you want to use GTask feature, you must start be getting Google OAuth2 crendentials.

First, go to [Google Cloud console](https://console.cloud.google.com/). Head to API and Servicies -> Identifiers -> Create new identifier -> OAuth Client ID. In general, the recommended way is to choose "Desktop Application" here, as limited peripherals only have limited accesses to your Google APIs.

Then, when your ids are created, download them and put them in `/in/credentials.json`.

## Tune your config as you want

Edit `/in/config.toml` config as you want. Here, you can enable/disable features.

