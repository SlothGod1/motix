//! Project settings: output resolution, frame rate, colour output, and how clips whose
//! size differs from the project are placed.

use motix_core::FrameRate;
use motix_core::limits::MAX_IMAGE_DIMENSION;
use std::fmt;

/// Output size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Why a setting was rejected, in words a person can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsError(pub String);

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SettingsError {}

/// Smallest accepted width or height.
pub const MIN_DIMENSION: u32 = 16;

impl Resolution {
    /// Validates a size.
    ///
    /// # Errors
    /// Each side must be between 16 and 16384 pixels and an even number: video
    /// encoders store colour at half resolution, so odd sizes can't be exported.
    pub fn new(width: u32, height: u32) -> Result<Self, SettingsError> {
        for (name, v) in [("Width", width), ("Height", height)] {
            if !(MIN_DIMENSION..=MAX_IMAGE_DIMENSION).contains(&v) {
                return Err(SettingsError(format!(
                    "{name} must be between {MIN_DIMENSION} and {MAX_IMAGE_DIMENSION} pixels."
                )));
            }
            if v % 2 != 0 {
                return Err(SettingsError(format!(
                    "{name} must be an even number (video formats need it) — try {}.",
                    v + 1
                )));
            }
        }
        Ok(Self { width, height })
    }

    /// Width ÷ height.
    #[must_use]
    pub fn aspect(self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }

    /// Simplest ratio, e.g. `"9:16"`, or `"1.90:1"` when the exact ratio is unwieldy.
    #[must_use]
    pub fn aspect_label(self) -> String {
        let g = gcd(self.width, self.height);
        let (w, h) = (self.width / g, self.height / g);
        if w <= 32 && h <= 32 {
            format!("{w}:{h}")
        } else if self.width >= self.height {
            format!("{:.2}:1", self.aspect())
        } else {
            format!("1:{:.2}", 1.0 / self.aspect())
        }
    }

    /// `"Vertical"`, `"Horizontal"` or `"Square"`.
    #[must_use]
    pub fn orientation(self) -> &'static str {
        match self.width.cmp(&self.height) {
            std::cmp::Ordering::Less => "Vertical",
            std::cmp::Ordering::Greater => "Horizontal",
            std::cmp::Ordering::Equal => "Square",
        }
    }

    /// Short quality name when there is a common one: `"HD"`, `"Full HD"`, `"4K UHD"`…
    #[must_use]
    pub fn quality_name(self) -> Option<&'static str> {
        let short = self.width.min(self.height);
        let long = self.width.max(self.height);
        match (long, short) {
            (1280, 720) => Some("HD"),
            (1920, 1080) => Some("Full HD"),
            (2560, 1440) => Some("QHD"),
            (3840, 2160) => Some("4K UHD"),
            (4096, 2160) => Some("DCI 4K"),
            (7680, 4320) => Some("8K UHD"),
            _ => None,
        }
    }

    /// Largest `(w, h)` with this aspect ratio inside `(max_w, max_h)` (screen points).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn fit_within(self, max_w: f32, max_h: f32) -> (f32, f32) {
        let a = self.aspect() as f32;
        if max_w <= 0.0 || max_h <= 0.0 {
            return (0.0, 0.0);
        }
        if max_w / max_h > a {
            (max_h * a, max_h)
        } else {
            (max_w, max_w / a)
        }
    }

    /// Safe-area guide as fractions of the frame: `(left, top, right, bottom)` insets.
    ///
    /// Vertical formats reserve more space at the bottom and right, where social apps
    /// draw captions, buttons and account names. These are *generic* guides; exact
    /// per-platform overlays come with the platform checker (IDEAS I-02).
    #[must_use]
    pub fn safe_insets(self) -> (f32, f32, f32, f32) {
        if self.height > self.width {
            (0.06, 0.10, 0.14, 0.22)
        } else {
            (0.05, 0.05, 0.05, 0.05)
        }
    }

    /// The same size turned 90°.
    #[must_use]
    pub fn swapped(self) -> Self {
        Self {
            width: self.height,
            height: self.width,
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// A named size offered as a shortcut; the user can always type any size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizePreset {
    /// Plain-language name.
    pub name: &'static str,
    /// Where it's typically used.
    pub used_for: &'static str,
    /// The size.
    pub resolution: Resolution,
}

const fn preset(name: &'static str, used_for: &'static str, width: u32, height: u32) -> SizePreset {
    SizePreset {
        name,
        used_for,
        resolution: Resolution { width, height },
    }
}

/// Common sizes, vertical first (the most common for social video).
pub const SIZE_PRESETS: [SizePreset; 9] = [
    preset("Vertical Full HD", "TikTok, Reels, Shorts", 1080, 1920),
    preset("Vertical 4K", "TikTok, Reels, Shorts (high quality)", 2160, 3840),
    preset("Portrait 4:5", "Instagram feed", 1080, 1350),
    preset("Square", "Feeds, X, Discord", 1080, 1080),
    preset("Full HD", "YouTube, most screens", 1920, 1080),
    preset("4K UHD", "YouTube 4K", 3840, 2160),
    preset("HD", "Smaller files", 1280, 720),
    preset("QHD", "YouTube 1440p", 2560, 1440),
    preset("Vertical HD", "Smaller vertical files", 720, 1280),
];

/// How the finished video's colour is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorOutput {
    /// Standard dynamic range, Rec.709 — plays correctly everywhere.
    Sdr,
    /// HDR10: PQ, Rec.2020, static metadata. What YouTube and phones call "HDR".
    Hdr10,
    /// HLG: Rec.2020, backwards-compatible HDR (common on phones).
    Hlg,
}

impl ColorOutput {
    /// Every option, in the order shown.
    pub const ALL: [Self; 3] = [Self::Sdr, Self::Hdr10, Self::Hlg];

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sdr => "SDR (Rec.709)",
            Self::Hdr10 => "HDR10 (PQ, Rec.2020)",
            Self::Hlg => "HLG (Rec.2020)",
        }
    }

    /// One-line explanation.
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::Sdr => "Standard video. Looks right on every screen and app.",
            Self::Hdr10 => "Brighter highlights and richer colour on HDR screens. Needs 10-bit.",
            Self::Hlg => "HDR that still looks acceptable on non-HDR screens. Needs 10-bit.",
        }
    }

    /// `true` for HDR outputs.
    #[must_use]
    pub const fn is_hdr(self) -> bool {
        !matches!(self, Self::Sdr)
    }
}

/// Where a clip goes when its size or shape differs from the project's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FitMode {
    /// Scale to fit inside the frame; nothing is cropped (bars may appear).
    #[default]
    Fit,
    /// Scale to fill the frame; edges may be cropped. Best for mixing vertical and horizontal clips.
    Fill,
    /// Stretch to exactly the frame (distorts if the shapes differ).
    Stretch,
    /// Keep original pixel size, centred.
    Original,
}

impl FitMode {
    /// Every option, in the order shown.
    pub const ALL: [Self; 4] = [Self::Fit, Self::Fill, Self::Stretch, Self::Original];

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fit => "Scale to fit",
            Self::Fill => "Scale to fill (crop)",
            Self::Stretch => "Stretch",
            Self::Original => "Original size",
        }
    }

    /// One-line explanation.
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::Fit => "Whole picture visible; black bars fill any gap.",
            Self::Fill => "Fills the frame; the edges that don't fit are cropped.",
            Self::Stretch => "Fills the frame exactly; may squash or stretch the picture.",
            Self::Original => "Pixel-for-pixel, centred; may be small or cropped.",
        }
    }

    /// Placement of a `source` picture in the `frame`, in frame pixels:
    /// `(x, y, width, height)` of the picture's rectangle (may extend outside the frame).
    #[must_use]
    pub fn place(self, source: Resolution, frame: Resolution) -> (f64, f64, f64, f64) {
        let (sw, sh) = (f64::from(source.width), f64::from(source.height));
        let (fw, fh) = (f64::from(frame.width), f64::from(frame.height));
        let (w, h) = match self {
            Self::Stretch => (fw, fh),
            Self::Original => (sw, sh),
            Self::Fit | Self::Fill => {
                let scale_w = fw / sw;
                let scale_h = fh / sh;
                let s = if self == Self::Fit {
                    scale_w.min(scale_h)
                } else {
                    scale_w.max(scale_h)
                };
                (sw * s, sh * s)
            }
        };
        ((fw - w) / 2.0, (fh - h) / 2.0, w, h)
    }
}

/// Settings of the whole project (the finished video).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectSettings {
    /// Output size.
    pub resolution: Resolution,
    /// Output and timeline frame rate.
    pub frame_rate: FrameRate,
    /// Colour of the finished video.
    pub color: ColorOutput,
    /// Bits per colour sample in the finished video (8 or 10; 12 later).
    pub bit_depth: u8,
    /// Placement for new clips whose size differs from the project.
    pub default_fit: FitMode,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            resolution: SIZE_PRESETS[0].resolution,
            frame_rate: FrameRate::FPS_30,
            color: ColorOutput::Sdr,
            bit_depth: 8,
            default_fit: FitMode::Fit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_validation() {
        assert!(Resolution::new(1080, 1920).is_ok());
        assert!(Resolution::new(1081, 1920).unwrap_err().0.contains("1082"));
        assert!(Resolution::new(0, 1920).is_err());
        assert!(Resolution::new(20_000, 1080).is_err());
        for p in SIZE_PRESETS {
            assert_eq!(
                Resolution::new(p.resolution.width, p.resolution.height),
                Ok(p.resolution)
            );
        }
    }

    #[test]
    fn labels() {
        let r = |w, h| Resolution::new(w, h).unwrap();
        assert_eq!(r(1080, 1920).aspect_label(), "9:16");
        assert_eq!(r(1920, 1080).aspect_label(), "16:9");
        assert_eq!(r(1080, 1350).aspect_label(), "4:5");
        assert_eq!(r(1998, 1080).aspect_label(), "1.85:1");
        assert_eq!(r(3840, 2160).quality_name(), Some("4K UHD"));
        assert_eq!(r(2160, 3840).quality_name(), Some("4K UHD"));
        assert_eq!(r(1080, 1920).orientation(), "Vertical");
        assert_eq!(r(1080, 1920).to_string(), "1080×1920");
        assert_eq!(r(1080, 1920).swapped(), r(1920, 1080));
    }

    #[test]
    fn fit_modes_place_pictures() {
        let uhd = Resolution::new(3840, 2160).unwrap();
        let vertical = Resolution::new(1080, 1920).unwrap();
        // Horizontal 4K into a vertical frame.
        let (x, y, w, h) = FitMode::Fit.place(uhd, vertical);
        assert!((w - 1080.0).abs() < 1e-9 && (h - 607.5).abs() < 1e-9);
        assert!(x.abs() < 1e-9 && (y - 656.25).abs() < 1e-9);
        let (x, _, w, h) = FitMode::Fill.place(uhd, vertical);
        assert!((h - 1920.0).abs() < 1e-9 && (w - 3_413.333_333).abs() < 1e-3 && x < 0.0);
        assert_eq!(FitMode::Stretch.place(uhd, vertical), (0.0, 0.0, 1080.0, 1920.0));
        assert_eq!(
            FitMode::Original.place(uhd, vertical),
            (-1380.0, -120.0, 3840.0, 2160.0)
        );
        // Same shape: every scaling mode fills exactly.
        let fhd = Resolution::new(1920, 1080).unwrap();
        for m in [FitMode::Fit, FitMode::Fill, FitMode::Stretch] {
            assert_eq!(m.place(uhd, fhd), (0.0, 0.0, 1920.0, 1080.0));
        }
    }

    #[test]
    fn fit_within_screen() {
        let r = Resolution::new(1080, 1920).unwrap();
        let (w, h) = r.fit_within(800.0, 600.0);
        assert!((h - 600.0).abs() < 0.01 && (w - 337.5).abs() < 0.01);
        assert_eq!(r.fit_within(0.0, 5.0), (0.0, 0.0));
    }
}
