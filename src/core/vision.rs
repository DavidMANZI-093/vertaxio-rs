use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::services::errors::XError;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct VisionPipeline {
    day_lower_bound: [i32; 3],
    day_upper_bound: [i32; 3],
    night_lower_bound: [i32; 3],
    night_upper_bound: [i32; 3],
    is_day_mode: Arc<AtomicBool>,
}

impl VisionPipeline {
    pub fn new(
        day_lower_bound: [i32; 3],
        day_upper_bound: [i32; 3],
        night_lower_bound: [i32; 3],
        night_upper_bound: [i32; 3],
        is_day_mode: Arc<AtomicBool>,
    ) -> Result<Self, XError> {
        crate::utils::logger::info("Initializing vision pipeline");

        Ok(Self {
            day_lower_bound,
            day_upper_bound,
            night_lower_bound,
            night_upper_bound,
            is_day_mode,
        })
    }

    #[inline(always)]
    fn rgb_to_hsv(r_u8: u8, g_u8: u8, b_u8: u8) -> (i32, i32, i32) {
        let r = r_u8 as i32;
        let g = g_u8 as i32;
        let b = b_u8 as i32;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let v = max;
        let s = if max == 0 { 0 } else { (delta * 255) / max };

        let mut h = 0;
        if delta != 0 {
            if max == r {
                h = (30 * (g - b)) / delta;
            } else if max == g {
                h = 60 + (30 * (b - r)) / delta;
            } else {
                h = 120 + (30 * (r - g)) / delta;
            }
            if h < 0 {
                h += 180;
            }
        }

        (h, s, v)
    }

    /// Processes a BGRA frame and returns bounding boxes of detected vertical rectangles.
    pub fn process_frame(&mut self, width: i32, height: i32, bgra_buffer: &mut [u8]) -> Result<Vec<Rect>, XError> {
        let area = (width * height) as usize;
        
        let is_day = self.is_day_mode.load(Ordering::Relaxed);
        let lower_bound = if is_day { self.day_lower_bound } else { self.night_lower_bound };
        let upper_bound = if is_day { self.day_upper_bound } else { self.night_upper_bound };

        // 1. Thresholding: Create a single byte mask buffer populated concurrently using Rayon SIMD
        let mut mask_buffer = vec![0u8; area];
        
        // bgra_buffer is &[u8] where each pixel is 4 bytes [B, G, R, A]
        // We iterate by 4-byte chunks exactly in parallel
        mask_buffer.par_iter_mut()
            .zip(bgra_buffer.par_chunks_exact(4))
            .for_each(|(mask_pixel, bgra)| {
                // Read B, G, R
                let b = bgra[0];
                let g = bgra[1];
                let r = bgra[2];

                // Convert to HSV
                let (h, s, v) = Self::rgb_to_hsv(r, g, b);

                // Apply Threshold bounds exactly like OpenCV `cv2.inRange`
                // Hue wraps at 360 (which is 180 in OpenCV). If Low H > High H, we must check both sides of the wrap!
                let h_match = if lower_bound[0] <= upper_bound[0] {
                    h >= lower_bound[0] && h <= upper_bound[0]
                } else {
                    h >= lower_bound[0] || h <= upper_bound[0] // It wrapped across the 0-degree red line
                };

                let is_match = h_match
                            && s >= lower_bound[1] && s <= upper_bound[1]
                            && v >= lower_bound[2] && v <= upper_bound[2];

                if is_match {
                    *mask_pixel = 255;
                }
            });

        // 2. Morphology operations (Open/Close noise reduction)
        // DILATION: Expands white pixels to fuse "burn holes" inside targets caused by loose saturation constraints.
        let mut dilated_mask = vec![0u8; area];
        
        let w = width as usize;
        let h = height as usize;

        // Process dilation in parallel. We process each row concurrently to avoid cross-thread indexing clashes.
        dilated_mask.par_chunks_exact_mut(w)
            .enumerate()
            .for_each(|(y, row_slice)| {
                for x in 0..w {
                    // Check 3x3 neighborhood. If ANY pixel is 255, this pixel becomes 255.
                    let mut is_white = false;
                    
                    let y_min = y.saturating_sub(1);
                    let y_max = (y + 1).min(h - 1);
                    let x_min = x.saturating_sub(1);
                    let x_max = (x + 1).min(w - 1);

                    'outer: for ny in y_min..=y_max {
                        let row_offset = ny * w;
                        for nx in x_min..=x_max {
                            if mask_buffer[row_offset + nx] == 255 {
                                is_white = true;
                                break 'outer;
                            }
                        }
                    }

                    if is_white {
                        row_slice[x] = 255;
                    }
                }
            });
        
        // 3. Contour & Bounding Box extraction
        // We use imageproc's find_contours. First we wrap our dilated mask in an ImageBuffer.
        let image_buf = image::ImageBuffer::<image::Luma<u8>, _>::from_raw(width as u32, height as u32, dilated_mask)
            .ok_or_else(|| XError::VisionError("Failed to wrap mask buffer in ImageBuffer".to_string()))?;

        let contours = imageproc::contours::find_contours::<i32>(&image_buf);
        
        let mut targets = Vec::new();

        for contour in contours {
            // Re-implement bounding_rect manually from the contour points
            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut max_y = i32::MIN;

            for point in contour.points {
                let px = point.x as i32;
                let py = point.y as i32;
                if px < min_x { min_x = px; }
                if px > max_x { max_x = px; }
                if py < min_y { min_y = py; }
                if py > max_y { max_y = py; }
            }

            let b_width = max_x - min_x;
            let b_height = max_y - min_y;

            // Prevent divide by zero
            if b_width == 0 || b_height == 0 {
                continue;
            }

            // Basic geometrical filtering for vertical rectangles (tall targets)
            let aspect_ratio = b_width as f32 / b_height as f32;
            let bounding_area = b_width * b_height;

            if bounding_area > 150 && aspect_ratio < 0.85 {
                targets.push(Rect {
                    x: min_x,
                    y: min_y,
                    width: b_width,
                    height: b_height,
                });
            }
        }

        Ok(targets)
    }
}
