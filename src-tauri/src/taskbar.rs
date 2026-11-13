//! Native download count, independent of the webview and still updated while the window is minimized.

use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct DownloadBadge {
    last_count: Option<usize>,
}

impl DownloadBadge {
    pub fn update(&mut self, handle: &AppHandle, count: usize) {
        if self.last_count == Some(count) {
            return;
        }
        let Some(window) = handle.get_webview_window("main") else {
            return;
        };
        #[cfg(windows)]
        let result = window.set_overlay_icon((count > 0).then(|| count_icon(count)));
        #[cfg(not(windows))]
        let result = window.set_badge_count((count > 0).then_some(count as i64));
        // Retry on the next tick if the native window was not ready yet.
        if result.is_ok() {
            self.last_count = Some(count);
        }
    }
}

/// Windows takes an icon, not a numeric badge. Draw a small, high-contrast count on a transparent background.
#[cfg(windows)]
fn count_icon(count: usize) -> tauri::image::Image<'static> {
    const SIZE: usize = 32;
    const DIGITS: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b010, 0b010, 0b010],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
    ];
    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };
    let scale = if label.len() > 2 { 2 } else { 3 };
    let width = (label.len() * 4 - 1) * scale;
    let left = (SIZE - width) / 2;
    let top = (SIZE - 5 * scale) / 2;
    let mut rgba = vec![0; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let distance = ((x as f32 - 15.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt();
            let alpha = ((16.0 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            let offset = (y * SIZE + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[0x0b, 0x3b, 0xd1, alpha]);
        }
    }
    for (index, character) in label.bytes().enumerate() {
        let glyph = if character == b'+' {
            [0, 0b010, 0b111, 0b010, 0]
        } else {
            DIGITS[(character - b'0') as usize]
        };
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = left + index * 4 * scale + column * scale + dx;
                        let y = top + row * scale + dy;
                        let offset = (y * SIZE + x) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&[255; 4]);
                    }
                }
            }
        }
    }
    tauri::image::Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

#[cfg(all(test, windows))]
mod tests {
    use super::count_icon;

    #[test]
    fn badge_icons_are_transparent_and_large_counts_are_capped() {
        for count in [1, 9, 10, 15, 99, 100, usize::MAX] {
            let icon = count_icon(count);
            assert_eq!((icon.width(), icon.height()), (32, 32));
            assert_eq!(icon.rgba().len(), 32 * 32 * 4);
            assert_eq!(icon.rgba()[3], 0, "transparent corner");
            assert!(
                icon.rgba().chunks_exact(4).any(|pixel| pixel == [255; 4]),
                "visible digits"
            );
        }
        assert_ne!(count_icon(1).rgba(), count_icon(2).rgba());
        assert_ne!(count_icon(9).rgba(), count_icon(10).rgba());
        assert_eq!(count_icon(100).rgba(), count_icon(usize::MAX).rgba());
    }
}
