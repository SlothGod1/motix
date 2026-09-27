//! Canvas (output frame) presets and safe-area guides.

/// An output canvas size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanvasPreset {
    /// Plain-language name.
    pub name: &'static str,
    /// Where it is typically used.
    pub used_for: &'static str,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl CanvasPreset {
    /// 9:16 — TikTok, Instagram Reels, YouTube Shorts.
    pub const VERTICAL: Self = Self {
        name: "Vertical 9:16",
        used_for: "TikTok, Reels, Shorts",
        width: 1080,
        height: 1920,
    };
    /// 4:5 — Instagram feed.
    pub const PORTRAIT: Self = Self {
        name: "Portrait 4:5",
        used_for: "Instagram feed",
        width: 1080,
        height: 1350,
    };
    /// 1:1.
    pub const SQUARE: Self = Self {
        name: "Square 1:1",
        used_for: "Feeds, X, Discord",
        width: 1080,
        height: 1080,
    };
    /// 16:9 — YouTube and most screens.
    pub const LANDSCAPE: Self = Self {
        name: "Landscape 16:9",
        used_for: "YouTube, desktop",
        width: 1920,
        height: 1080,
    };

    /// All presets, in the order shown to users.
    pub const ALL: [Self; 4] = [Self::VERTICAL, Self::PORTRAIT, Self::SQUARE, Self::LANDSCAPE];

    /// Width ÷ height.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn aspect(self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Largest `(w, h)` with this aspect ratio that fits inside `(max_w, max_h)`.
    #[must_use]
    pub fn fit_within(self, max_w: f32, max_h: f32) -> (f32, f32) {
        let a = self.aspect();
        if max_w <= 0.0 || max_h <= 0.0 {
            return (0.0, 0.0);
        }
        if max_w / max_h > a {
            (max_h * a, max_h)
        } else {
            (max_w, max_w / a)
        }
    }

    /// Safe-area guide as fractions of the canvas: `(left, top, right, bottom)` insets.
    ///
    /// Vertical formats reserve more space at the bottom and right, where social apps
    /// draw captions, buttons and account names. These are *generic* guides; exact
    /// per-platform overlays are planned with the platform compliance checker (IDEAS I-02).
    #[must_use]
    pub fn safe_insets(self) -> (f32, f32, f32, f32) {
        if self.height > self.width {
            (0.06, 0.10, 0.14, 0.22)
        } else {
            (0.05, 0.05, 0.05, 0.05)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_preserve_aspect_and_bounds() {
        for p in CanvasPreset::ALL {
            for (w, h) in [(800.0, 600.0), (300.0, 900.0), (1.0, 1.0), (1920.0, 1080.0)] {
                let (fw, fh) = p.fit_within(w, h);
                assert!(fw <= w + 0.01 && fh <= h + 0.01, "{} in {w}x{h}", p.name);
                assert!((fw / fh - p.aspect()).abs() < 0.001);
                assert!((fw - w).abs() < 0.01 || (fh - h).abs() < 0.01, "must touch one edge");
            }
        }
        assert_eq!(CanvasPreset::VERTICAL.fit_within(0.0, 10.0), (0.0, 0.0));
    }

    #[test]
    fn safe_insets_leave_room() {
        for p in CanvasPreset::ALL {
            let (l, t, r, b) = p.safe_insets();
            assert!(l + r < 0.5 && t + b < 0.5);
        }
    }
}
