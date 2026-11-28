//! Safe, idiomatic Rust abstractions over the C library (1:1 with the C API).
//!
//! All FFI calls of the project live here. C items are always written with
//! their `ffi::` prefix: every crossing into C is visible at a glance.
//! Elsewhere, `unsafe` code only calls the `unsafe fn`s below (whose `# Safety`
//! section states what the caller must guarantee), or `libc::raise` in the simulator.
//!
//! Rust names are the C ones without their prefix (`led_canvas_fill` -> `Canvas::fill`)
//! nor `get_` (`led_matrix_get_brightness` -> `RgbLedMatrix::brightness`).
//!
//! The whole C API (`led-matrix-c.h`) is wrapped, except the 3 functions built around
//! the library's own `--led-*` command-line flags (the app is configured by its config file):
//! - `led_matrix_create_from_options` and `led_matrix_create_from_options_const_argv`:
//!   create the matrix from `argv` (C++ would keep pointers into the arg strings),
//! - `led_matrix_print_flags`: prints the help of these flags to a C `FILE*`.

// The wrapper exposes the full C API, even the parts the app doesn't use yet
#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char};
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::ffi::{self, Color};

// ===== Options =====

// About both option structs below:
// **`0` / `false` / `None` means "library default", not zero**: the C API only
// copies non-zero fields over its own defaults (`OPT_COPY_IF_SET` in `led-matrix-c.cc`).
// E.g. `brightness: 0` actually runs with 100, `gpio_slowdown: 0` with 1 (2 on a Pi 4).
// So `Default::default()` (all zeros) simply means "all library defaults".
// Fields follow the order of the C structs.

/// Matrix options (C `RGBLedMatrixOptions`). See the note above about zeros.
///
/// The C++ matrix keeps pointers to the strings, so [`RgbLedMatrix`] stores these
/// options for its whole life.
#[derive(Debug, Clone, Default)]
pub struct MatrixOptions {
    /// Wiring between the Pi and the panel (e.g. "adafruit-hat").
    /// library default: "regular"
    pub hardware_mapping: Option<CString>,
    /// library default: 32
    pub rows: i32,
    /// library default: 32
    pub cols: i32,
    /// library default: 1
    pub chain_length: i32,
    /// library default: 1
    pub parallel: i32,
    /// PWM bits per color (1..11): fewer = less CPU, higher refresh rate.
    /// library default: 11
    pub pwm_bits: i32,
    /// On-time of the lowest PWM bit, in ns: higher = better colors, lower refresh rate.
    /// library default: 130
    pub pwm_lsb_nanoseconds: i32,
    /// Lower PWM bits time-dithered, for a higher refresh rate.
    /// library default: 0
    pub pwm_dither_bits: i32,
    /// library default: 100
    pub brightness: i32,
    /// 0 = progressive, 1 = interlaced.
    /// library default: 0
    pub scan_mode: i32,
    /// How rows are addressed: 0 = direct, other values for some panels (e.g. 64x64).
    /// library default: 0
    pub row_address_type: i32,
    /// 0 = direct, other values for panels with special multiplexing (e.g. 1:8).
    /// library default: 0
    pub multiplexing: i32,
    /// Don't use the hardware to create the pulses.
    /// library default: false
    pub disable_hardware_pulsing: bool,
    /// Prints the refresh rate on the terminal.
    /// library default: false
    pub show_refresh_rate: bool,
    /// For panels with inverted colors.
    /// library default: false
    pub inverse_colors: bool,
    /// Order of the colors on the panel, if it's not RGB.
    /// library default: "RGB"
    pub led_rgb_sequence: Option<CString>,
    /// Pixel mappers to apply, separated by `;` (e.g. "Rotate:90").
    /// library default: none
    pub pixel_mapper_config: Option<CString>,
    /// For panels needing an init sequence (e.g. "FM6126A").
    /// library default: none
    pub panel_type: Option<CString>,
    /// Caps the refresh rate, in Hz: steadier on a loaded system.
    /// library default: 0 (no limit)
    pub limit_refresh_rate_hz: i32,
    /// Sleeps instead of busy waiting when the refresh rate is limited: less precise,
    /// but frees the CPU (useful on single-core boards).
    /// library default: false
    pub disable_busy_waiting: bool,
}

/// Runtime options (C `RGBLedRuntimeOptions`). See the note above about zeros.
///
/// Only read while the matrix is created (C++ keeps no pointer to them),
/// so [`RgbLedMatrix::new`] just borrows them.
///
/// No `daemon` field on purpose: it stays 0 (zeroed struct). The other values break
/// the app: -1 never starts the refresh thread (`swap_on_vsync` then returns NULL),
/// and 1 forks, which loses the tokio threads. systemd already runs the app in the background.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOptions {
    /// library default: 1 (2 on a Pi 4)
    pub gpio_slowdown: i32,
    /// 0 = RP1 RIO, 1 = RP1 PIO (Pi 5 only)
    pub rp1_pio: i32,
    /// None: library default (on)
    pub drop_privileges: Option<bool>,
    /// library default: true
    pub do_gpio_init: bool,
    /// library default: "daemon"
    pub drop_priv_user: Option<CString>,
    /// library default: "daemon"
    pub drop_priv_group: Option<CString>,
}

// ===== Canvas =====

/// A handle to a drawing surface (C `LedCanvas`), owned by the C matrix.
///
/// Invariant: `ptr` is never null, and stays valid as long as the matrix lives
/// (see [`RgbLedMatrix::create_offscreen_canvas`]).
///
/// Pixels drawn out of the canvas are ignored by C (no error, no crash).
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

    /// Fills the `width` x `height` rectangle whose top-left corner is (x, y).
    pub fn subfill(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe {
            ffi::led_canvas_subfill(self.ptr, x, y, width, height, color.r, color.g, color.b);
        }
    }

    /// Sets an individual pixel's color.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe { ffi::led_canvas_set_pixel(self.ptr, x, y, color.r, color.g, color.b) }
    }

    /// Copies `colors` (row after row) to the `width` x `height` rectangle whose
    /// top-left corner is (x, y).
    ///
    /// # Panics
    /// If `colors` doesn't hold exactly `width * height` colors.
    pub fn set_pixels(&mut self, x: i32, y: i32, width: i32, height: i32, colors: &[Color]) {
        assert_eq!(
            buffer_len(width, height, 1),
            Some(colors.len()),
            "expected {width}x{height} colors"
        );
        // SAFETY: `ptr` is valid (see `Canvas`). C reads `width * height` colors,
        // exactly the length of `colors` (checked above), and never writes them:
        // `cast_mut` is only needed because the C signature lacks a `const`.
        unsafe {
            ffi::led_canvas_set_pixels(self.ptr, x, y, width, height, colors.as_ptr().cast_mut());
        }
    }

    /// Draws a `width` x `height` image whose top-left corner is (x, y).
    ///
    /// `buffer` holds the pixels row after row, 3 bytes each: R, G, B (or B, G, R
    /// if `is_bgr`), e.g. `RgbImage::as_raw()` of the `image` crate.
    /// (x, y) can be negative: the image is cropped at the edges.
    ///
    /// # Panics
    /// If `buffer` doesn't hold exactly `3 * width * height` bytes.
    pub fn set_image(
        &mut self,
        x: i32,
        y: i32,
        buffer: &[u8],
        width: i32,
        height: i32,
        is_bgr: bool,
    ) {
        // C would silently draw nothing: a wrong size is a bug, make it visible
        assert_eq!(
            buffer_len(width, height, 3),
            Some(buffer.len()),
            "expected {width}x{height} pixels of 3 bytes"
        );
        // SAFETY: `ptr` is valid (see `Canvas`). C reads at most `buffer.len()` bytes
        // (it checks this length itself too).
        unsafe {
            ffi::set_image(
                self.ptr,
                x,
                y,
                buffer.as_ptr(),
                buffer.len(),
                width,
                height,
                c_char::from(is_bgr),
            );
        }
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

    /// Draws UTF-8 text with a loaded BDF font, from left to right.
    ///
    /// `y` is the baseline of the text (not its top). `kerning_offset` adds pixels
    /// between the characters (can be negative, 0 for the font's spacing).
    /// Returns the width of the text drawn, in pixels.
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
        kerning_offset: i32,
    ) -> Result<i32> {
        let c_text = CString::new(text).context("invalid text for a C string")?;
        // SAFETY: both pointers are valid (see `Canvas` and `Font`), and `c_text`
        // lives until the end of the function, so during the whole call.
        let width = unsafe {
            ffi::draw_text(
                self.ptr,
                font.ptr,
                x,
                y,
                color.r,
                color.g,
                color.b,
                c_text.as_ptr(),
                kerning_offset,
            )
        };
        Ok(width)
    }

    /// Same as [`Canvas::draw_text`], but from top to bottom (one character per line).
    /// Returns the height of the text drawn, in pixels.
    ///
    /// # Errors
    /// If `text` contains a NUL byte (it can't be converted to a C string).
    pub fn vertical_draw_text(
        &mut self,
        font: &Font,
        x: i32,
        y: i32,
        color: Color,
        text: &str,
        kerning_offset: i32,
    ) -> Result<i32> {
        let c_text = CString::new(text).context("invalid text for a C string")?;
        // SAFETY: same as in `draw_text`.
        let height = unsafe {
            ffi::vertical_draw_text(
                self.ptr,
                font.ptr,
                x,
                y,
                color.r,
                color.g,
                color.b,
                c_text.as_ptr(),
                kerning_offset,
            )
        };
        Ok(height)
    }

    /// Draws the outline of a circle centered at (x, y).
    pub fn draw_circle(&mut self, x: i32, y: i32, radius: i32, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe { ffi::draw_circle(self.ptr, x, y, radius, color.r, color.g, color.b) }
    }

    /// Draws a line from (x0, y0) to (x1, y1).
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        // SAFETY: `ptr` is valid (see `Canvas`).
        unsafe { ffi::draw_line(self.ptr, x0, y0, x1, y1, color.r, color.g, color.b) }
    }
}

// ===== Font =====

/// A BDF font loaded by the C library, to draw text with [`Canvas::draw_text`].
///
/// Invariant: `ptr` is never null (C allocates fonts with C++ `new`, which can't
/// return null), and is only freed by `drop`.
pub struct Font {
    ptr: *mut ffi::LedFont,
}

impl Font {
    /// Loads a BDF font from the specified file path.
    ///
    /// # Errors
    /// If `path` contains a NUL byte, or if the C library can't load the font.
    pub fn load(path: &Path) -> Result<Self> {
        // Raw bytes: a path isn't always valid UTF-8
        let c_path =
            CString::new(path.as_os_str().as_encoded_bytes()).context("invalid font path")?;
        // SAFETY: `c_path` is a valid C string, alive during the whole call.
        let ptr = unsafe { ffi::load_font(c_path.as_ptr()) };
        // Owned from here: if loading failed below, `drop` frees it
        let font = Self { ptr };

        // C ignores load errors: a font that failed to load keeps its height of -1
        if font.height() < 0 {
            bail!("failed to load font from {}", path.display());
        }
        Ok(font)
    }

    /// Creates a new font: this one with a 1-pixel border around each character.
    ///
    /// To outline text, as the C++ examples do (same `y` for both):
    /// 1. draw it with the outline font at `x - 1`, with `kerning_offset - 2`
    ///    (its characters are 2 pixels wider),
    /// 2. draw it with this font at `x`, with `kerning_offset`, on top.
    pub fn create_outline(&self) -> Self {
        // SAFETY: `ptr` is valid (see `Font`). C only reads it, and returns a new,
        // independent font, owned by the returned `Font` (freed by its `drop`).
        let ptr = unsafe { ffi::create_outline_font(self.ptr) };
        Self { ptr }
    }

    /// Distance from the top of the font to its baseline, in pixels.
    pub fn baseline(&self) -> i32 {
        // SAFETY: `ptr` is valid (see `Font`).
        unsafe { ffi::baseline_font(self.ptr) }
    }

    /// Height of the font, in pixels.
    pub fn height(&self) -> i32 {
        // SAFETY: `ptr` is valid (see `Font`).
        unsafe { ffi::height_font(self.ptr) }
    }

    /// Width of `c` in pixels, or `None` if the font has no glyph for it.
    pub fn character_width(&self, c: char) -> Option<i32> {
        // SAFETY: `ptr` is valid (see `Font`).
        let width = unsafe { ffi::character_width_font(self.ptr, c.into()) };
        // C returns -1 for a missing glyph
        (width >= 0).then_some(width)
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // SAFETY: `ptr` is valid (see `Font`), and freed only once, here.
        unsafe { ffi::delete_font(self.ptr) }
    }
}

// ===== RgbLedMatrix =====

/// The LED matrix itself (C `RGBLedMatrix`): owns the hardware and its refresh thread.
///
/// Pure 1:1 wrapper: it knows nothing about how the app draws (see `display.rs`).
///
/// Invariant: `ptr` is never null (constructors return an error instead), and is only
/// freed by `drop`.
pub struct RgbLedMatrix {
    ptr: *mut ffi::RGBLedMatrix,
    /// Options the matrix was created with. The C++ matrix keeps raw pointers to
    /// their strings (and reads `led_rgb_sequence` again on each canvas creation),
    /// so they must live as long as the matrix.
    options: MatrixOptions,
}

// SAFETY: `Send` only allows *moving* the matrix to another thread, never using it
// from two threads at once (that would need `Sync`, not implemented). The C++ matrix
// isn't tied to the thread that created it (its refresh thread is internal).
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

        opts.hardware_mapping = opt_c_ptr(options.hardware_mapping.as_deref());
        opts.rows = options.rows;
        opts.cols = options.cols;
        opts.chain_length = options.chain_length;
        opts.parallel = options.parallel;
        opts.pwm_bits = options.pwm_bits;
        opts.pwm_lsb_nanoseconds = options.pwm_lsb_nanoseconds;
        opts.pwm_dither_bits = options.pwm_dither_bits;
        opts.brightness = options.brightness;
        opts.scan_mode = options.scan_mode;
        opts.row_address_type = options.row_address_type;
        opts.multiplexing = options.multiplexing;
        opts.disable_hardware_pulsing = options.disable_hardware_pulsing;
        opts.show_refresh_rate = options.show_refresh_rate;
        opts.inverse_colors = options.inverse_colors;
        opts.led_rgb_sequence = opt_c_ptr(options.led_rgb_sequence.as_deref());
        opts.pixel_mapper_config = opt_c_ptr(options.pixel_mapper_config.as_deref());
        opts.panel_type = opt_c_ptr(options.panel_type.as_deref());
        opts.limit_refresh_rate_hz = options.limit_refresh_rate_hz;
        opts.disable_busy_waiting = options.disable_busy_waiting;

        rt_opts.gpio_slowdown = rt_options.gpio_slowdown;
        rt_opts.rp1_pio = rt_options.rp1_pio;
        rt_opts.drop_privileges = match rt_options.drop_privileges {
            None => 0,
            Some(true) => 1,
            Some(false) => -1,
        };
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

        // Moving `options` doesn't move the strings' heap buffers: the pointers stay valid
        Ok(Self { ptr, options })
    }

    /// Simple form of [`RgbLedMatrix::new`]: only the panels' layout, everything else
    /// keeps the library defaults (root privileges are dropped to the `daemon` user).
    /// Requires root privileges on the Pi.
    ///
    /// # Errors
    /// If the C library fails to create the matrix (usually: not run as root).
    pub fn create(rows: i32, chained: i32, parallel: i32) -> Result<Self> {
        // SAFETY: only plain integers are passed.
        let ptr = unsafe { ffi::led_matrix_create(rows, chained, parallel) };
        if ptr.is_null() {
            bail!("failed to create the RGB LED matrix (not running as root?)");
        }

        // No string given to C++, so none to keep alive
        Ok(Self {
            ptr,
            options: MatrixOptions::default(),
        })
    }

    /// Returns the canvas currently displayed, to draw on without double buffering:
    /// changes show up right away. Blocks until the next vertical sync.
    ///
    /// # Safety
    /// Same as [`RgbLedMatrix::create_offscreen_canvas`]: the returned canvas must not
    /// be used after `self` is dropped. Also, after a swap, it can point to the same
    /// buffer as another `Canvas`: two such handles must never be used from two
    /// threads at once.
    pub unsafe fn canvas(&mut self) -> Canvas {
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`).
        let ptr = unsafe { ffi::led_matrix_get_canvas(self.ptr) };
        Canvas { ptr }
    }

    /// Creates a new offscreen canvas, for double buffering with [`RgbLedMatrix::swap_on_vsync`].
    ///
    /// Each call allocates a new framebuffer, only freed when the matrix is deleted:
    /// create it once and reuse it, never on every frame (memory leak).
    ///
    /// # Safety
    /// The returned canvas must not be used after `self` is dropped: the C library
    /// frees it in `led_matrix_delete`. A lifetime can't express this here (the owner
    /// would have to store a reference to its own field), so the caller guarantees it.
    pub unsafe fn create_offscreen_canvas(&mut self) -> Canvas {
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

/// Frees the C++ matrix and stops its refresh thread (the panel turns off).
/// Runs before the fields are dropped: `options` outlives the C++ matrix.
impl Drop for RgbLedMatrix {
    fn drop(&mut self) {
        // SAFETY: `ptr` is valid (see `RgbLedMatrix`), and freed only once, here.
        unsafe { ffi::led_matrix_delete(self.ptr) }
    }
}

// ===== Helpers =====

/// Pointer to the string, or null for `None` (= "library default" for the C API).
fn opt_c_ptr(s: Option<&CStr>) -> *const c_char {
    s.map_or(std::ptr::null(), CStr::as_ptr)
}

/// Length of a buffer of `width` x `height` pixels, `per_pixel` items each.
///
/// `None` if a size is negative, or if the length overflows the C `int`
/// the library computes it with.
fn buffer_len(width: i32, height: i32, per_pixel: i32) -> Option<usize> {
    if width < 0 || height < 0 {
        return None;
    }
    let len = width.checked_mul(height)?.checked_mul(per_pixel)?;
    usize::try_from(len).ok()
}
