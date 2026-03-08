use crate::core::vision::Rect;
use crate::services::errors::XError;
use minifb::{Scale, Window, WindowOptions};

const WINDOW_NAME: &str = "Vertaxio Debug";

pub struct DebugWindow {
    enabled: bool,
    window: Option<Window>,
    draw_buffer: Vec<u32>,
    target_fps: usize,
}

impl DebugWindow {
    pub fn new(enabled: bool, target_fps: usize) -> Self {
        Self {
            enabled,
            window: None,
            draw_buffer: Vec::new(),
            target_fps,
        }
    }

    pub fn draw(
        &mut self,
        width: i32,
        height: i32,
        bgra_buffer: &mut [u8],
        targets: &[Rect],
    ) -> Result<(), XError> {
        if !self.enabled {
            return Ok(());
        }

        let w = width as usize;
        let h = height as usize;

        if self.window.is_none() {
            let mut window = Window::new(
                WINDOW_NAME,
                w,
                h,
                WindowOptions {
                    scale: Scale::X1,
                    resize: true,
                    ..WindowOptions::default()
                },
            )
            .map_err(|e| XError::VisionError(format!("Failed to create debug window: {}", e)))?;

            window.set_target_fps(self.target_fps);
            self.window = Some(window);
            self.draw_buffer = vec![0u32; w * h];
        }

        // minifb expects a buffer of u32 pixels (0RGB).
        // Our input bgra_buffer is [B, G, R, A] (4 bytes per pixel).
        // We efficiently bitwise shift the bytes to convert to 0RGB.
        for (i, chunk) in bgra_buffer.chunks_exact(4).enumerate() {
            let b = chunk[0] as u32;
            let g = chunk[1] as u32;
            let r = chunk[2] as u32;
            // a = chunk[3] (unused for minifb)
            self.draw_buffer[i] = (r << 16) | (g << 8) | b;
        }

        // Draw green bounding boxes over each target found
        let color_green: u32 = 0x00_00_FF_00; // 0RGB -> Green
        let line_thickness = 2;

        for rect in targets {
            let rx = rect.x.max(0) as usize;
            let ry = rect.y.max(0) as usize;
            let rw = rect.width as usize;
            let rh = rect.height as usize;

            // Don't draw if completely out of bounds
            if rx >= w || ry >= h {
                continue;
            }

            // Ensure we don't draw outside the buffer
            let end_x = (rx + rw).min(w - 1);
            let end_y = (ry + rh).min(h - 1);

            // Draw Top and Bottom lines
            for t in 0..line_thickness {
                let current_top_y = (ry + t).min(h - 1);
                let current_bottom_y = (end_y.saturating_sub(t)).max(0);

                for x in rx..=end_x {
                    self.draw_buffer[current_top_y * w + x] = color_green;
                    self.draw_buffer[current_bottom_y * w + x] = color_green;
                }
            }

            // Draw Left and Right lines
            for t in 0..line_thickness {
                let current_left_x = (rx + t).min(w - 1);
                let current_right_x = (end_x.saturating_sub(t)).max(0);

                for y in ry..=end_y {
                    self.draw_buffer[y * w + current_left_x] = color_green;
                    self.draw_buffer[y * w + current_right_x] = color_green;
                }
            }
        }

        if let Some(window) = &mut self.window {
            window
                .update_with_buffer(&self.draw_buffer, w, h)
                .map_err(|e| {
                    XError::VisionError(format!("Failed to update window buffer: {}", e))
                })?;

            if !window.is_open() {
                self.enabled = false;
                crate::utils::logger::info("Debug window closed by user.");
            }
        }

        Ok(())
    }

    /// Answers OS thread pings during idle logic
    pub fn update(&mut self) {
        if let Some(window) = &mut self.window {
            window.update();
            if !window.is_open() {
                self.enabled = false;
            }
        }
    }
}
