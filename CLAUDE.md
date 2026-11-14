I learned Rust on my own following Rust book, and this project is to practice and get used to the language.
The aim is to control a 32x64 RGB LED Matrix using a rapsberry pi zero W. I'll display text, custom-ui (rectangles, polygons...), images and animations.
This project uses rpi-rgb-led-matrix C library from hzeller. I chose to generate the bindings using bindgen, instead of using existing crates.
To handle the graphics, I use the embedded-graphics crate and its companion crates (embedded-xx). For the images, I use the image crate.

When coding in this project, always produce idiomatic, well documented and code easily maintainable for a human.
Also take into account low capabilities of the rpi zero W, and keep the code very efficient.
As I'm not a pro with Rust yet, don't hesitate to propose better alternatives, and to insert some learning tips, good practices in your answers.
