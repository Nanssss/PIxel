//! The app's screen: the LED matrix used with double buffering.
//!
//! This is the policy layer built on the 1:1 wrapper: it owns the matrix and
//! its single offscreen canvas, and contains no FFI call.
//!
//! Usage: draw on the `Matrix` with `use embedded_graphics::prelude::*;` and
//! `drawable.draw(matrix)`, then call `matrix.show()` to display the frame.

use core::convert::Infallible;
use std::ffi::CString;
use std::os::unix::fs::MetadataExt;
use anyhow::{Context, Result};
use tracing::{info, warn};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use super::wrapper::{Canvas, MatrixOptions, RgbLedMatrix, RuntimeOptions};

/// The LED matrix as the app sees it: draw on it, then [`Matrix::show`] the frame.
pub struct Matrix {
    /// Hidden buffer we draw on before displaying it (double buffering).
    /// Fields drop in declaration order: the canvas goes before the matrix that owns it.
    offscreen: Canvas,
    rgb_matrix: RgbLedMatrix,
}

impl Matrix {
    /// Creates the matrix for this project's panel (32x64, Adafruit HAT).
    /// Requires root privileges on the Pi.
    ///
    /// # Errors
    /// If the C library fails to create the matrix (usually: not run as root).
    pub fn new() -> Result<Self> {
        // This project's panel. Fields not listed keep the library defaults
        // (`..Default::default()` fills the rest with zeros = library defaults).
        let options = MatrixOptions {
            rows: 32,
            cols: 64,
            brightness: 100,
            // `c"..."` is a C-string literal (`&'static CStr`), `to_owned` makes a `CString`
            hardware_mapping: Some(c"adafruit-hat".to_owned()),
            ..Default::default()
        };

        // Root is only needed to create the matrix: see `runtime_options`
        let rt_options = runtime_options()?;

        let mut rgb_matrix = RgbLedMatrix::new(options, &rt_options)?;

        // Safety: the canvas is stored next to `rgb_matrix` and never leaves
        // `Matrix`, so it can't be used after the matrix is dropped.
        let offscreen = unsafe { rgb_matrix.create_offscreen_canvas() };

        Ok(Self { offscreen, rgb_matrix })
    }

    /// Displays everything drawn on the matrix since the last call, on the next vertical sync.
    ///
    /// Drawing goes to a hidden buffer (double buffering, no flickering). The swap gives
    /// back the previously displayed buffer, which is cleared and becomes the next hidden
    /// one, so each frame starts from a black screen. Only two buffers ever exist:
    ///
    /// ```text
    ///          frame 1          frame 2          frame 3
    /// shown:   [A]    swap ->   [B]    swap ->   [A]
    /// hidden:  [B] <- draw      [A] <- draw      [B] <- draw
    /// ```
    pub fn show(&mut self) {
        self.rgb_matrix.swap_on_vsync(&mut self.offscreen);
        self.offscreen.clear();
    }
}

/// The matrix is drawn on like a canvas: everything goes to its hidden buffer,
/// and nothing is visible until `Matrix::show` is called.
impl OriginDimensions for Matrix {
    fn size(&self) -> Size {
        OriginDimensions::size(&self.offscreen)
    }
}

impl DrawTarget for Matrix {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.offscreen.draw_iter(pixels)
    }

    /// Fills the whole panel with one color, using the fast C library call.
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        // Can't directly use .clear() because it's already a canvas function
        // In this case we want to map it to led_canvas_fill
        DrawTarget::clear(&mut self.offscreen, color)
    }
}

/// Runtime options of the matrix
///
/// The app is started as root, which the matrix needs to access the GPIO. Right
/// after creating it, the C library drops root privileges and switches to another
/// user: by default `daemon`, which can't write the files of the app.
/// We switch instead to the owner of the executable, i.e. the user who deployed it,
/// and who also owns those files.
///
/// # Errors
/// If the executable file can't be read.
fn runtime_options() -> Result<RuntimeOptions> {
    let exe = std::env::current_exe().context("failed to get the executable path")?;
    let metadata = std::fs::metadata(&exe)
        .with_context(|| format!("failed to read the owner of {exe:?}"))?;

    // `MetadataExt` (Unix only) gives the numeric user and group ids of the file
    let (uid, gid) = (metadata.uid(), metadata.gid());

    // The C library reads "0" as a name, not as root's id: the lookup would fail and
    // the app would keep running as root anyway, after printing an error. Make it explicit.
    if uid == 0 || gid == 0 {
        warn!("The executable belongs to root: privileges are kept, the app runs as root");
        return Ok(RuntimeOptions {
            drop_privileges: -1,    // -1: disabled (0 would mean "library default": on)
            ..Default::default()
        });
    }

    info!("Root privileges will be dropped to uid {uid}, gid {gid} (owner of the executable)");

    // The C library accepts a user (group) name or its numeric id. A number never
    // contains a NUL byte, so `CString::new` can't fail here.
    Ok(RuntimeOptions {
        drop_priv_user:  Some(CString::new(uid.to_string())?),
        drop_priv_group: Some(CString::new(gid.to_string())?),
        ..Default::default()
    })
}
