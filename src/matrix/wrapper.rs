//! Safe, idiomatic Rust abstractions over the C library (1:1 with the C API).
//!
//! All FFI calls of the project live here. C items are always written with
//! their `ffi::` prefix: every crossing into C is visible at a glance.
//! The only `unsafe` code outside of this file is calling the `unsafe fn`s below,
//! whose `# Safety` section states what the caller must guarantee.

// The wrapper exposes the full C API, even the parts the app doesn't use yet.
#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char};

use anyhow::{Context, Result, bail};

use super::ffi::{self, Color};

// About both option structs below:
// **`0` / `false` / `None` means "library default", not zero**: the C API only
// copies non-zero fields over its own defaults (`OPT_COPY_IF_SET` in `led-matrix-c.cc`).
// E.g. `brightness: 0` actually runs with 100, `gpio_slowdown: 0` with 1 (2 on a Pi 4).
// So `Default::default()` (all zeros) simply means "all library defaults".

/// Matrix options (C `RGBLedMatrixOptions`, subset). See the note above about zeros.
///
/// Strings are `CString` (NUL-terminated, like C's `const char*`): the C++ matrix
/// keeps pointers to them, so [`RgbLedMatrix`] stores these options for its whole life.
#[derive(Debug, Clone, Default)]
pub struct MatrixOptions {
    pub rows: i32,                         // library default: 32
    pub cols: i32,                         // library default: 32
    pub chain_length: i32,                 // library default: 1
    pub parallel: i32,                     // library default: 1
    pub brightness: i32,                   // library default: 100
    pub hardware_mapping: Option<CString>, // library default: "regular"
    pub led_rgb_sequence: Option<CString>, // library default: "RGB"
}

/// Runtime options (C `RGBLedRuntimeOptions`). See the note above about zeros.
///
/// Only read while the matrix is created (C++ keeps no pointer to them),
/// so [`RgbLedMatrix::new`] just borrows them.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOptions {
    pub gpio_slowdown: i32,               // library default: 1 (2 on a Pi 4)
    pub rp1_pio: i32,                     // 0 = RP1 RIO, 1 = RP1 PIO (Pi 5 only)
    pub daemon: i32,                      // -1 disabled, 0 off (default), 1 on
    pub drop_privileges: i32,             // -1 disabled, 0 off, 1 on (default)
    pub do_gpio_init: bool,               // library default: true
    pub drop_priv_user: Option<CString>,  // library default: "daemon"
    pub drop_priv_group: Option<CString>, // library default: "daemon"
}

/// A handle to a drawing surface (C `LedCanvas`), owned by the C matrix.
///
/// Invariant: `ptr` is never null, and stays valid as long as the matrix lives
/// (see [`RgbLedMatrix::create_offscreen_canvas`]).
pub struct Canvas {
    ptr: *mut ffi::LedCanvas,
}

// SAFETY: same reasoning as for `RgbLedMatrix`: a canvas is plain framebuffer
// memory, not tied to a thread, so moving the handle to another thread is fine.
unsafe impl Send for Canvas {}

impl Canvas {
    /// Clears the canvas (turns all pixels black).
    pub fn clear(&mut self) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe { ffi::led_canvas_clear(self.ptr) }
    }

    /// Fills the entire canvas with a solid RGB color.
    pub fn fill(&mut self, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe { ffi::led_canvas_fill(self.ptr, color.r, color.g, color.b) }
    }

    /// Sets an individual pixel's color.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`). Out-of-bounds pixels are ignored by C.
        unsafe { ffi::led_canvas_set_pixel(self.ptr, x, y, color.r, color.g, color.b) }
    }

    /// Returns the (width, height) dimensions of the canvas in pixels.
    pub fn size(&self) -> (i32, i32) {
        let mut width = 0;
        let mut height = 0;
        // SAFETY: `ptr` is valid (see `Canvas`), and C writes the size into two
        // local variables that outlive the call.
        unsafe {
            ffi::led_canvas_get_size(self.ptr, &mut width, &mut height);
        }
        (width, height)
    }

    /// Draws UTF-8 text onto the canvas using a loaded BDF font.
    ///
    /// # Errors
    /// If `text` contains a NUL byte (it can't be converted to a C string).
    pub fn draw_text(
        &mut self,
        font: &Font,
        x: i32,
        y: i32,
        color: Color,
        text: &str,
    ) -> Result<i32> {
        let c_text = CString::new(text).context("invalid text for a C string")?;
        // SAFETY: both pointers are valid (see `Canvas` and `Font`), and `c_text`
        // lives until the end of the function, so during the whole call.
        let res = unsafe {
            ffi::draw_text(
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

/// A BDF font loaded by the C library, to draw text with [`Canvas::draw_text`].
///
/// Invariant: `ptr` is never null (`load` returns an error instead), and is only
/// freed by `drop`.
pub struct Font {
    ptr: *mut ffi::LedFont,
}

impl Font {
    /// Loads a BDF font from the specified file path.
    ///
    /// # Errors
    /// If `path` contains a NUL byte, or if the C library can't load the font.
    pub fn load(path: &str) -> Result<Self> {
        let c_path = CString::new(path).context("invalid font path")?;
        // SAFETY: `c_path` is a valid C string, alive during the whole call.
        let ptr = unsafe { ffi::load_font(c_path.as_ptr()) };
        if ptr.is_null() {
            // `bail!(...)` is anyhow's shortcut for `return Err(anyhow!(...))`
            bail!("failed to load font from {path}");
        }
        Ok(Self { ptr })
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // SAFETY: `ptr` is valid (see `Font`), and freed only once, here.
        unsafe { ffi::delete_font(self.ptr) }
    }
}

/// The LED matrix itself (C `RGBLedMatrix`): owns the hardware and its refresh thread.
///
/// Pure 1:1 wrapper: it knows nothing about how the app draws (see `display.rs`).
///
/// Invariant: `ptr` is never null (`new` returns an error instead), and is only
/// freed by `drop`.
pub struct RgbLedMatrix {
    ptr: *mut ffi::RGBLedMatrix,
    /// Options the matrix was created with. The C++ matrix keeps raw pointers to
    /// their strings (and reads `led_rgb_sequence` again on each canvas creation),
    /// so they must live as long as the matrix.
    options: MatrixOptions,
}

// SAFETY: `Send` only allows *moving* the matrix to another thread (e.g. when a
// tokio task migrates), never using it from two threads at once: that would need
// `Sync`, which we don't implement. The C++ matrix isn't tied to the thread that
// created it (its refresh thread is internal), so moving the handle is fine.
// Raw pointers are `!Send` by default, hence this explicit (unsafe) promise.
unsafe impl Send for RgbLedMatrix {}

impl RgbLedMatrix {
    /// Creates the matrix with the given options. Requires root privileges on the Pi.
    ///
    /// `options` is taken by value (C++ keeps pointers to its strings, so it's
    /// stored in `Self`), `rt_options` only borrowed (read during creation only).
    ///
    /// # Errors
    /// If the C library fails to create the matrix (usually: not run as root).
    pub fn new(options: MatrixOptions, rt_options: &RuntimeOptions) -> Result<Self> {
        // SAFETY: the C API expects zeroed structs ("0 = library default"),
        // which is a valid value for these plain C structs (ints, bools, null pointers).
        let mut opts: ffi::RGBLedMatrixOptions = unsafe { std::mem::zeroed() };
        // SAFETY: same as above.
        let mut rt_opts: ffi::RGBLedRuntimeOptions = unsafe { std::mem::zeroed() };

        // Populate standard options. `as_deref` turns a `&Option<CString>` into the
        // `Option<&CStr>` that `opt_c_ptr` takes.
        opts.rows = options.rows;
        opts.cols = options.cols;
        opts.chain_length = options.chain_length;
        opts.parallel = options.parallel;
        opts.brightness = options.brightness;
        opts.hardware_mapping = opt_c_ptr(options.hardware_mapping.as_deref());
        opts.led_rgb_sequence = opt_c_ptr(options.led_rgb_sequence.as_deref());

        // Populate runtime options
        rt_opts.gpio_slowdown = rt_options.gpio_slowdown;
        rt_opts.rp1_pio = rt_options.rp1_pio;
        rt_opts.daemon = rt_options.daemon;
        rt_opts.drop_privileges = rt_options.drop_privileges;
        rt_opts.do_gpio_init = rt_options.do_gpio_init;
        rt_opts.drop_priv_user = opt_c_ptr(rt_options.drop_priv_user.as_deref());
        rt_opts.drop_priv_group = opt_c_ptr(rt_options.drop_priv_group.as_deref());

        // SAFETY: both structs are valid and outlive the call. The strings they
        // point to belong to `options` (moved into `Self` below, as C++ keeps
        // these pointers) and `rt_options` (borrowed, so alive during the call).
        let ptr =
            unsafe { ffi::led_matrix_create_from_options_and_rt_options(&mut opts, &mut rt_opts) };
        if ptr.is_null() {
            bail!("failed to create the RGB LED matrix (not running as root?)");
        }

        // Moving `options` only moves the `CString` handles, not their heap
        // buffers: the pointers given to C++ above stay valid.
        Ok(Self { ptr, options })
    }

    /// Creates a new offscreen canvas, for double buffering with [`RgbLedMatrix::swap_on_vsync`].
    ///
    /// Each call allocates a new framebuffer, only freed when the matrix is deleted:
    /// create it once and reuse it, never on every frame (memory leak).
    ///
    /// # Safety
    /// The returned canvas must not be used after `self` is dropped: the C library
    /// frees it in `led_matrix_delete`. This can't be expressed with a lifetime here
    /// (the owner would have to store a reference to its own field), so the caller
    /// guarantees it.
    pub unsafe fn create_offscreen_canvas(&mut self) -> Canvas {
        // In edition 2024, the body of an `unsafe fn` still needs `unsafe` blocks.
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`).
        let ptr = unsafe { ffi::led_matrix_create_offscreen_canvas(self.ptr) };
        Canvas { ptr }
    }

    /// Displays `canvas` on the next vertical sync (blocks until then).
    ///
    /// `canvas` must come from [`RgbLedMatrix::create_offscreen_canvas`] of this matrix.
    /// Afterwards, it points to the previously displayed buffer (content untouched),
    /// ready to be drawn on as the next offscreen canvas.
    pub fn swap_on_vsync(&mut self, canvas: &mut Canvas) {
        // SAFETY: both pointers are valid (see `RgbLedMatrix` and `Canvas`). C gives
        // back the other buffer of the same matrix, so the canvas stays valid.
        canvas.ptr = unsafe { ffi::led_matrix_swap_on_vsync(self.ptr, canvas.ptr) };
    }

    /// Gets current brightness (1..100).
    pub fn brightness(&self) -> u8 {
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`).
        unsafe { ffi::led_matrix_get_brightness(self.ptr) }
    }

    /// Sets panel brightness (1..100).
    pub fn set_brightness(&mut self, brightness: u8) {
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`).
        unsafe { ffi::led_matrix_set_brightness(self.ptr, brightness) }
    }
}

/// Automatically cleans up matrix memory and stops hardware threads safely on exit.
/// Rust runs this before dropping the fields, so `options` outlives the C++ matrix.
impl Drop for RgbLedMatrix {
    fn drop(&mut self) {
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`), and freed only once, here.
        unsafe { ffi::led_matrix_delete(self.ptr) }
    }
}

/// Pointer to the string, or null for `None` (= "library default" for the C API).
// `Option<&CStr>` rather than `&Option<CString>`: the idiomatic way to take an
// optional borrowed value, which doesn't force the caller to own an `Option`.
fn opt_c_ptr(s: Option<&CStr>) -> *const c_char {
    s.map_or(std::ptr::null(), CStr::as_ptr)
}
