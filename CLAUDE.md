I learned Rust on my own following Rust book, and this project is to practice and get used to the language.
The aim is to control a 32x64 RGB LED Matrix using a rapsberry pi zero W. I'll display text, custom-ui (rectangles, polygons...), images and animations.
This project uses rpi-rgb-led-matrix C library from hzeller. I chose to generate the bindings using bindgen, instead of using existing crates.
To handle the graphics, I use the embedded-graphics crate and its companion crates (embedded-xx). For the images, I'll start with tinybmp and use the image crate if needed.
As I'm not a pro with Rust yet, don't hesitate to propose better alternatives, and to insert some learning tips and good practices in your answers.

When coding in this project:
- follow Rust idioms
- use short comments explaining the "why" and write important remarks in notes.md
- keep in mind the code will run on a Raspberry Pi Zero W, keep the code efficient
