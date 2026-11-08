#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#[allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;


// ============================================================================
// 2. SAFE, IDIOMATIC RUST ABSTRACTIONS
// ============================================================================

/// Configuration builder for the LED Matrix.
#[derive(Debug, Clone)]
pub struct MatrixOptions {
    pub rows: i32,
    pub cols: i32,
    pub chain_length: i32,
    pub parallel: i32,
    pub brightness: i32,
    pub hardware_mapping: Option<String>,
    pub led_rgb_sequence: Option<String>,
    pub gpio_slowdown: i32,
    pub rp1_pio: i32,
}

impl Default for MatrixOptions {
    fn default() -> Self {
        Self {
            rows: 32,
            cols: 64,
            chain_length: 1,
            parallel: 1,
            brightness: 100,
            hardware_mapping: Some("adafruit-hat".to_string()),
            led_rgb_sequence: None,
            gpio_slowdown: 0,
            rp1_pio: 0,
        }
    }
}

/// A handle to a drawing surface (either active display or offscreen buffer).
pub struct Canvas {
    ptr: *mut LedCanvas,
}

impl Canvas {
    /// Clears the canvas (turns all pixels black).
    pub fn clear(&mut self) {
        unsafe { led_canvas_clear(self.ptr) }
    }

    /// Fills the entire canvas with a solid RGB color.
    pub fn fill(&mut self, color: Color) {
        unsafe { led_canvas_fill(self.ptr, color.r, color.g, color.b) }
    }

    /// Sets an individual pixel's color.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        unsafe { led_canvas_set_pixel(self.ptr, x, y, color.r, color.g, color.b) }
    }

    /// Returns the (width, height) dimensions of the canvas in pixels.
    pub fn size(&self) -> (i32, i32) {
        let mut width = 0;
        let mut height = 0;
        unsafe {
            led_canvas_get_size(self.ptr, &mut width, &mut height);
        }
        (width, height)
    }

    /// Draws UTF-8 text onto the canvas using a loaded BDF font.
    pub fn draw_text(&mut self, font: &Font, x: i32, y: i32, color: Color, text: &str) -> Result<i32, String> {
        let c_text = CString::new(text).map_err(|e| format!("Invalid text for C string: {}", e))?;
        let res = unsafe {
            draw_text(
                self.ptr,
                font.ptr,
                x,
                y,
                color.r,
                color.g,
                color.b,
                c_text.as_ptr(),
                0,
            )
        };
        Ok(res)
    }
}

/// A loaded BDF font wrapper.
pub struct Font {
    ptr: *mut LedFont,
}

impl Font {
    /// Loads a BDF font from the specified file path.
    pub fn load(path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid font path: {}", e))?;
        let ptr = unsafe { load_font(c_path.as_ptr()) };
        if ptr.is_null() {
            return Err(format!("Failed to load font from: {}", path));
        }
        Ok(Self { ptr })
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                delete_font(self.ptr);
            }
        }
    }
}

/// The main LED Matrix controller managing hardware resources and threads.
pub struct Matrix {
    ptr: *mut RGBLedMatrix,
}

// Safety: The underlying C++ library handles thread synchronization for its display loop.
unsafe impl Send for Matrix {}

impl Matrix {
    /// Initializes the RGB matrix with user options. Requires root privileges on the Pi.
    pub fn new(options: MatrixOptions) -> Result<Self, &'static str> {
        unsafe {
            // 1. Zero-initialize options structs
            let mut opts: RGBLedMatrixOptions = std::mem::zeroed();
            let mut rt_opts: RGBLedRuntimeOptions = std::mem::zeroed();

            // Populate standard options
            opts.rows = options.rows;
            opts.cols = options.cols;
            opts.chain_length = options.chain_length;
            opts.parallel = options.parallel;
            opts.brightness = options.brightness;

            // Handle optional strings safely by leaking owned CStrings for FFI lifetime
            let mapping_c;
            if let Some(ref m) = options.hardware_mapping {
                mapping_c = CString::new(m.clone()).map_err(|_| "Invalid mapping string")?;
                opts.hardware_mapping = mapping_c.as_ptr();
            }

            let seq_c;
            if let Some(ref s) = options.led_rgb_sequence {
                seq_c = CString::new(s.clone()).map_err(|_| "Invalid RGB sequence string")?;
                opts.led_rgb_sequence = seq_c.as_ptr();
            }

            // Populate runtime options
            rt_opts.gpio_slowdown = options.gpio_slowdown;
            rt_opts.rp1_pio = options.rp1_pio;
            rt_opts.do_gpio_init = true;
            rt_opts.daemon = 0;           // Run in foreground by default
            rt_opts.drop_privileges = 1;  // Drop root privileges after init

            // 2. Instantiate matrix via C library
            let matrix_ptr = led_matrix_create_from_options_and_rt_options(&mut opts, &mut rt_opts);
            if matrix_ptr.is_null() {
                return Err("Failed to create RGB LED Matrix. Ensure you are running with root privileges.");
            }

            Ok(Self { ptr: matrix_ptr })
        }
    }

    /// Returns a canvas handle for direct immediate drawing.
    pub fn canvas(&mut self) -> Canvas {
        let canvas_ptr = unsafe { led_matrix_get_canvas(self.ptr) };
        Canvas { ptr: canvas_ptr }
    }

    /// Creates an offscreen canvas for flicker-free double buffering.
    pub fn create_offscreen_canvas(&mut self) -> Canvas {
        let canvas_ptr = unsafe { led_matrix_create_offscreen_canvas(self.ptr) };
        Canvas { ptr: canvas_ptr }
    }

    /// Swaps the offscreen buffer with the active display on vertical sync.
    /// Returns the old active canvas to be reused as the next offscreen buffer.
    pub fn swap_on_vsync(&mut self, mut offscreen: Canvas) -> Canvas {
        let new_offscreen_ptr = unsafe { led_matrix_swap_on_vsync(self.ptr, offscreen.ptr) };
        offscreen.ptr = new_offscreen_ptr;
        offscreen
    }

    /// Gets current brightness (1..100).
    pub fn brightness(&self) -> u8 {
        unsafe { led_matrix_get_brightness(self.ptr) }
    }

    /// Sets panel brightness (1..100).
    pub fn set_brightness(&mut self, brightness: u8) {
        unsafe { led_matrix_set_brightness(self.ptr, brightness) }
    }
}

/// Automatically cleans up matrix memory and stops hardware threads safely on exit.
impl Drop for Matrix {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                led_matrix_delete(self.ptr);
            }
        }
    }
}
