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
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use tracing::{info, warn};

use super::wrapper::{Canvas, MatrixOptions, RgbLedMatrix, RuntimeOptions};
use super::{PANEL_HEIGHT, PANEL_WIDTH};
use crate::paths::EXECUTABLE_DIR;

/// The LED matrix as the app sees it: draw on it, then [`Matrix::show`] the frame.
pub struct Matrix {
    /// Hidden buffer we draw on before displaying it (double buffering).
    /// Declared first: fields drop in order, so the canvas goes before its matrix.
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
        // This project's panel. Fields not listed keep the library defaults.
        let options = MatrixOptions {
            // Small constants: `as` can't truncate them
            rows: PANEL_HEIGHT as i32,
            cols: PANEL_WIDTH as i32,
            brightness: 100,
            hardware_mapping: Some(c"adafruit-hat".to_owned()),
            ..Default::default()
        };

        // Root is only needed to create the matrix: see `runtime_options`
        let rt_options = runtime_options()?;

        let mut rgb_matrix = RgbLedMatrix::new(options, &rt_options)?;

        // SAFETY: the canvas is stored next to `rgb_matrix` and never leaves
        // `Matrix`, so it can't be used after the matrix is dropped.
        let offscreen = unsafe { rgb_matrix.create_offscreen_canvas() };

        Ok(Self {
            offscreen,
            rgb_matrix,
        })
    }

    /// Displays everything drawn on the matrix since the last call, on the next vertical sync.
    ///
    /// The swap gives back the previously displayed buffer, which is cleared and becomes
    /// the next hidden one: each frame starts from a black screen.
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

/// Drawing goes to the hidden buffer: nothing is visible until `Matrix::show`.
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
        // Not `self.offscreen.clear()`: that is `Canvas::clear`, which only clears to black
        DrawTarget::clear(&mut self.offscreen, color)
    }
}

/// Runtime options of the matrix: which user the app runs as once started.
///
/// The app is started as root, which the matrix needs to access the GPIO. Right
/// after creating it, the C library drops root privileges for the `daemon` user by
/// default, which can't write the files of the app. We switch instead to the owner
/// of the app directory: the user who deployed it, and owns its files (config, logs...).
///
/// # Errors
/// If the app directory can't be read.
fn runtime_options() -> Result<RuntimeOptions> {
    let metadata = EXECUTABLE_DIR
        .metadata()
        .with_context(|| format!("failed to read the owner of {}", EXECUTABLE_DIR.display()))?;

    let (uid, gid) = (metadata.uid(), metadata.gid());

    // The C library reads "0" as a name, not as root's id: the lookup would fail and
    // the app would keep running as root anyway, after printing an error. Make it explicit.
    if uid == 0 || gid == 0 {
        warn!("The app directory belongs to root: privileges are kept, the app runs as root");
        return Ok(RuntimeOptions {
            drop_privileges: Some(false),
            ..Default::default()
        });
    }

    info!("Root privileges will be dropped to uid {uid}, gid {gid} (owner of the app directory)");

    // The C library takes a name or a numeric id. A number has no NUL byte:
    // `CString::new` can't fail here.
    Ok(RuntimeOptions {
        drop_priv_user: Some(CString::new(uid.to_string())?),
        drop_priv_group: Some(CString::new(gid.to_string())?),
        ..Default::default()
    })
}
