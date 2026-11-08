use crate::app::states::matrix::matrix_wrapper::{Color, Matrix, MatrixOptions};

pub mod matrix_wrapper;

// Re-export public types
// pub use matrix_wrapper::{Matrix, MatrixOptions, Canvas, Font, Color};

pub struct AppMatrix {
    pub matrix:     Matrix,
}

impl AppMatrix {
    pub fn new() -> Self {
        let options = MatrixOptions {
            rows: 32,
            cols: 64,
            brightness: 100,
            hardware_mapping: Some("adafruit-hat".to_string()),
            ..Default::default()
        };

        let matrix = Matrix::new(options).unwrap();
        
        AppMatrix {
            matrix,
        }
    }

    pub fn fill(&mut self, color: Color) {
        let mut canvas = self.matrix.create_offscreen_canvas();
        canvas.fill(color);
        self.matrix.swap_on_vsync(canvas);
    }
}
