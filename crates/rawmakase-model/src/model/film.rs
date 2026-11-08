//! RAWmakase-native film controls, with independent saved effect switches.
//! Serialized legacy names are retained; controls use normalized UI values.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
mod profiles;
pub use profiles::{CompressionFormat, GrainFormat};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilmEffects {
    #[serde(skip_serializing_if = "enabled")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "enabled")]
    pub halation_enabled: bool,
    /// Impact. Older recipes called this Amount.
    pub halation_amount: f32,
    /// Local Diffusion. Older recipes called this Spread.
    pub halation_spread: f32,
    /// Additional source threshold, retained independently of Source Limiter.
    pub halation_threshold: f32,
    pub halation_hue: f32,
    pub halation_source_limiter: f32,
    pub halation_background_gain: f32,
    pub halation_smoothness: f32,
    pub halation_global_diffusion: f32,
    pub halation_amplify: f32,
    pub halation_blue_compensation: f32,
    #[serde(skip_serializing_if = "enabled")]
    pub bloom_enabled: bool,
    pub bloom_amount: f32,
    pub bloom_spread: f32,
    pub bloom_threshold: f32,
    pub bloom_protection: f32,
    pub bloom_saturation: f32,
    pub bloom_source_limiter: f32,
    pub bloom_details: f32,
    pub bloom_amplify: f32,
    #[serde(skip_serializing_if = "enabled")]
    pub grain_enabled: bool,
    #[serde(skip_serializing_if = "disabled")]
    pub compression_enabled: bool,
    pub grain_amount: f32,
    pub grain_size: f32,
    pub grain_shadows: f32,
    pub grain_midtones: f32,
    pub grain_highlights: f32,
    pub grain_color: f32,
    pub grain_resolution: f32,
    pub compression_impact: f32,
    pub compression_range: f32,
    pub compression_white_point: f32,
    pub compression_color_density: f32,
    pub grain_seed: u64,
}
impl Default for FilmEffects {
    fn default() -> Self {
        // Neutral starting recipe: every operator is bypassed. Shape controls are
        // independent authored starting points, not a stock or vendor match.
        Self {
            enabled: true,
            halation_enabled: true,
            halation_amount: 0.,
            halation_spread: 0.24,
            halation_threshold: 0.76,
            halation_hue: 0.23,
            halation_source_limiter: 0.5,
            halation_background_gain: 0.,
            halation_smoothness: 0.68,
            halation_global_diffusion: 0.,
            halation_amplify: 0.,
            halation_blue_compensation: 0.5,
            bloom_enabled: true,
            bloom_amount: 0.,
            bloom_spread: 0.32,
            bloom_threshold: 0.8,
            bloom_protection: 0.72,
            bloom_saturation: 0.5,
            bloom_source_limiter: 0.5,
            bloom_details: 0.7,
            bloom_amplify: 0.,
            grain_enabled: true,
            compression_enabled: false,
            grain_amount: 0.,
            grain_size: 0.28,
            grain_shadows: 0.5,
            grain_midtones: 0.75,
            grain_highlights: 0.3,
            grain_color: 0.,
            grain_resolution: 0.8,
            compression_impact: 0.,
            compression_range: 0.5,
            compression_white_point: 1.,
            compression_color_density: 0.,
            grain_seed: 42,
        }
    }
}
impl FilmEffects {
    pub fn active(self) -> bool {
        self.diffusion_active() || self.grain_active() || self.compression_active()
    }
    pub fn diffusion_active(self) -> bool {
        self.halation_active() || self.bloom_active()
    }
    pub fn grain_active(self) -> bool {
        self.enabled && self.grain_enabled && self.grain_amount > 0.
    }
    pub fn compression_active(self) -> bool {
        self.enabled && self.compression_enabled
    }
    pub fn copy_diffusion_from(&mut self, from: Self) {
        self.copy_halation_from(from);
        self.copy_bloom_from(from);
    }
    pub fn copy_grain_from(&mut self, from: Self) {
        self.enabled = from.enabled;
        self.grain_amount = from.grain_amount;
        self.grain_size = from.grain_size;
        self.grain_shadows = from.grain_shadows;
        self.grain_midtones = from.grain_midtones;
        self.grain_highlights = from.grain_highlights;
        self.grain_color = from.grain_color;
        self.grain_resolution = from.grain_resolution;
        self.grain_enabled = from.grain_enabled;
        self.grain_seed = from.grain_seed;
    }
    pub fn grain_values(self) -> [f32; 7] {
        [
            self.grain_amount,
            self.grain_size,
            self.grain_shadows,
            self.grain_midtones,
            self.grain_highlights,
            self.grain_color,
            self.grain_resolution,
        ]
    }
    pub fn copy_compression_from(&mut self, from: Self) {
        self.enabled = from.enabled;
        self.compression_impact = from.compression_impact;
        self.compression_range = from.compression_range;
        self.compression_white_point = from.compression_white_point;
        self.compression_color_density = from.compression_color_density;
        self.compression_enabled = from.compression_enabled;
    }
    pub fn compression_values(self) -> [f32; 4] {
        [
            self.compression_impact,
            self.compression_range,
            self.compression_white_point,
            self.compression_color_density,
        ]
    }
    pub fn halation_active(self) -> bool {
        self.enabled && self.halation_enabled && self.halation_amount > 0.
    }
    pub fn bloom_active(self) -> bool {
        self.enabled && self.bloom_enabled && self.bloom_amount > 0.
    }
    /// A selected child's settings carry its shared section state, as other panels do.
    pub fn copy_halation_from(&mut self, from: Self) {
        self.enabled = from.enabled;
        self.halation_enabled = from.halation_enabled;
        self.halation_amount = from.halation_amount;
        self.halation_spread = from.halation_spread;
        self.halation_threshold = from.halation_threshold;
        self.halation_hue = from.halation_hue;
        self.halation_source_limiter = from.halation_source_limiter;
        self.halation_background_gain = from.halation_background_gain;
        self.halation_smoothness = from.halation_smoothness;
        self.halation_global_diffusion = from.halation_global_diffusion;
        self.halation_amplify = from.halation_amplify;
        self.halation_blue_compensation = from.halation_blue_compensation;
    }
    pub fn copy_bloom_from(&mut self, from: Self) {
        self.enabled = from.enabled;
        self.bloom_enabled = from.bloom_enabled;
        self.bloom_amount = from.bloom_amount;
        self.bloom_spread = from.bloom_spread;
        self.bloom_threshold = from.bloom_threshold;
        self.bloom_protection = from.bloom_protection;
        self.bloom_saturation = from.bloom_saturation;
        self.bloom_source_limiter = from.bloom_source_limiter;
        self.bloom_details = from.bloom_details;
        self.bloom_amplify = from.bloom_amplify;
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.values()
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v)),
            "Invalid film effect range"
        );
        ensure!(
            self.grain_values()
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v)),
            "Invalid film texture range"
        );
        ensure!(
            [
                self.compression_impact,
                self.compression_range,
                self.compression_color_density
            ]
            .iter()
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                && self.compression_white_point.is_finite()
                && (0.5..=1.2).contains(&self.compression_white_point),
            "Invalid film compression range"
        );
        Ok(())
    }
    pub fn halation_values(self) -> [f32; 10] {
        [
            self.halation_source_limiter,
            self.halation_background_gain,
            self.halation_smoothness,
            self.halation_spread,
            self.halation_global_diffusion,
            self.halation_amplify,
            self.halation_hue,
            self.halation_blue_compensation,
            self.halation_amount,
            self.halation_threshold,
        ]
    }
    pub fn bloom_values(self) -> [f32; 8] {
        [
            self.bloom_threshold,
            self.bloom_source_limiter,
            self.bloom_details,
            self.bloom_spread,
            self.bloom_amplify,
            self.bloom_protection,
            self.bloom_saturation,
            self.bloom_amount,
        ]
    }
    pub fn values(self) -> [f32; 18] {
        let h = self.halation_values();
        let b = self.bloom_values();
        std::array::from_fn(|i| if i < h.len() { h[i] } else { b[i - h.len()] })
    }
    pub fn interpolate(self, to: Self, amount: f32) -> Self {
        let blend = |a: f32, b: f32| (a + (b - a) * amount).clamp(0., 1.);
        Self {
            enabled: if amount > 0. {
                to.enabled
            } else {
                self.enabled
            },
            halation_enabled: if amount > 0. {
                to.halation_enabled
            } else {
                self.halation_enabled
            },
            bloom_enabled: if amount > 0. {
                to.bloom_enabled
            } else {
                self.bloom_enabled
            },
            halation_amount: blend(self.halation_amount, to.halation_amount),
            halation_spread: blend(self.halation_spread, to.halation_spread),
            halation_threshold: blend(self.halation_threshold, to.halation_threshold),
            halation_hue: blend(self.halation_hue, to.halation_hue),
            halation_source_limiter: blend(
                self.halation_source_limiter,
                to.halation_source_limiter,
            ),
            halation_background_gain: blend(
                self.halation_background_gain,
                to.halation_background_gain,
            ),
            halation_smoothness: blend(self.halation_smoothness, to.halation_smoothness),
            halation_global_diffusion: blend(
                self.halation_global_diffusion,
                to.halation_global_diffusion,
            ),
            halation_amplify: blend(self.halation_amplify, to.halation_amplify),
            halation_blue_compensation: blend(
                self.halation_blue_compensation,
                to.halation_blue_compensation,
            ),
            bloom_amount: blend(self.bloom_amount, to.bloom_amount),
            bloom_spread: blend(self.bloom_spread, to.bloom_spread),
            bloom_threshold: blend(self.bloom_threshold, to.bloom_threshold),
            bloom_protection: blend(self.bloom_protection, to.bloom_protection),
            bloom_saturation: blend(self.bloom_saturation, to.bloom_saturation),
            bloom_source_limiter: blend(self.bloom_source_limiter, to.bloom_source_limiter),
            bloom_details: blend(self.bloom_details, to.bloom_details),
            bloom_amplify: blend(self.bloom_amplify, to.bloom_amplify),
            grain_enabled: if amount > 0. {
                to.grain_enabled
            } else {
                self.grain_enabled
            },
            compression_enabled: if amount > 0. {
                to.compression_enabled
            } else {
                self.compression_enabled
            },
            grain_amount: (self.grain_amount + (to.grain_amount - self.grain_amount) * amount)
                .clamp(0.0, 1.0),
            grain_size: (self.grain_size + (to.grain_size - self.grain_size) * amount)
                .clamp(0.0, 1.0),
            grain_shadows: (self.grain_shadows + (to.grain_shadows - self.grain_shadows) * amount)
                .clamp(0.0, 1.0),
            grain_midtones: (self.grain_midtones
                + (to.grain_midtones - self.grain_midtones) * amount)
                .clamp(0.0, 1.0),
            grain_highlights: (self.grain_highlights
                + (to.grain_highlights - self.grain_highlights) * amount)
                .clamp(0.0, 1.0),
            grain_color: (self.grain_color + (to.grain_color - self.grain_color) * amount)
                .clamp(0.0, 1.0),
            grain_resolution: (self.grain_resolution
                + (to.grain_resolution - self.grain_resolution) * amount)
                .clamp(0.0, 1.0),
            compression_impact: (self.compression_impact
                + (to.compression_impact - self.compression_impact) * amount)
                .clamp(0.0, 1.0),
            compression_range: (self.compression_range
                + (to.compression_range - self.compression_range) * amount)
                .clamp(0.0, 1.0),
            compression_white_point: (self.compression_white_point
                + (to.compression_white_point - self.compression_white_point) * amount)
                .clamp(0.5, 1.2),
            compression_color_density: (self.compression_color_density
                + (to.compression_color_density - self.compression_color_density) * amount)
                .clamp(0.0, 1.0),
            grain_seed: if amount > 0. {
                to.grain_seed
            } else {
                self.grain_seed
            },
        }
    }
    /// Numeric format starting points, independent from the rendering operators.
    fn halation_profile(format: FilmFormat) -> Self {
        let scale = format.coarseness();
        let open = if format.open_backing() { 1. } else { 0. };
        Self {
            halation_amount: 0.14 + 0.12 * scale + 0.10 * open,
            halation_spread: 0.12 + 0.44 * scale + 0.09 * open,
            halation_threshold: 0.76 - 0.08 * scale,
            halation_hue: 0.23,
            halation_source_limiter: 0.62 - 0.14 * scale - 0.18 * open,
            halation_background_gain: 0.22 + 0.12 * scale,
            halation_smoothness: 0.68,
            halation_global_diffusion: 0.04 + 0.16 * scale + 0.08 * open,
            halation_amplify: 0.18 + 0.08 * scale + 0.12 * open,
            halation_blue_compensation: 0.48,
            ..Self::default()
        }
    }
    fn bloom_profile(format: FilmFormat) -> Self {
        let scale = format.coarseness();
        Self {
            bloom_amount: 0.10 + 0.20 * scale,
            bloom_spread: 0.20 + 0.52 * scale,
            bloom_threshold: 0.82 - 0.08 * scale,
            bloom_protection: 0.72,
            bloom_saturation: 0.45,
            bloom_source_limiter: 0.48,
            bloom_details: 0.78 - 0.28 * scale,
            bloom_amplify: 0.16 + 0.14 * scale,
            ..Self::default()
        }
    }
    pub fn apply_halation_preset(&mut self, format: FilmFormat) {
        let enabled = self.enabled;
        self.copy_halation_from(Self::halation_profile(format));
        self.enabled = enabled;
    }
    pub fn apply_bloom_preset(&mut self, format: FilmFormat) {
        let enabled = self.enabled;
        self.copy_bloom_from(Self::bloom_profile(format));
        self.enabled = enabled;
    }
    pub fn halation_format(self) -> Option<FilmFormat> {
        FilmFormat::ALL
            .into_iter()
            .find(|f| self.halation_values() == Self::halation_profile(*f).halation_values())
    }
    pub fn bloom_format(self) -> Option<FilmFormat> {
        FilmFormat::BLOOM
            .into_iter()
            .find(|f| self.bloom_values() == Self::bloom_profile(*f).bloom_values())
    }
    pub fn preset(format: FilmFormat) -> Self {
        let mut f = Self::default();
        f.apply_halation_preset(format);
        f.apply_bloom_preset(format);
        f
    }
}
fn disabled(value: &bool) -> bool {
    !*value
}
fn enabled(value: &bool) -> bool {
    *value
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmFormat {
    Small8,
    Small8NoRemjet,
    Small16,
    Small16NoRemjet,
    Standard35,
    Strong35,
    Large65,
    Large65NoRemjet,
}
impl FilmFormat {
    /// A design coordinate: smaller formats get broader, stronger effects.
    /// These profiles are original aesthetic starting points, not measurements.
    fn coarseness(self) -> f32 {
        match self {
            Self::Small8 | Self::Small8NoRemjet => 1.,
            Self::Small16 | Self::Small16NoRemjet => 2. / 3.,
            Self::Standard35 | Self::Strong35 => 1. / 3.,
            Self::Large65 | Self::Large65NoRemjet => 0.,
        }
    }
    fn open_backing(self) -> bool {
        matches!(
            self,
            Self::Small8NoRemjet | Self::Small16NoRemjet | Self::Strong35 | Self::Large65NoRemjet
        )
    }

    pub const ALL: [Self; 8] = [
        Self::Small8,
        Self::Small8NoRemjet,
        Self::Small16,
        Self::Small16NoRemjet,
        Self::Standard35,
        Self::Strong35,
        Self::Large65,
        Self::Large65NoRemjet,
    ];
    pub const BLOOM: [Self; 4] = [Self::Small8, Self::Small16, Self::Standard35, Self::Large65];
    pub fn label(self) -> &'static str {
        match self {
            Self::Small8 => "8 mm / Super 8",
            Self::Small8NoRemjet => "8 mm / Super 8 No Remjet",
            Self::Small16 => "16 mm / Super 16",
            Self::Small16NoRemjet => "16 mm / Super 16 No Remjet",
            Self::Standard35 => "35 mm / Super 35",
            Self::Strong35 => "35 mm / Super 35 No Remjet",
            Self::Large65 => "65 mm / 70 mm",
            Self::Large65NoRemjet => "65 mm / 70 mm No Remjet",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grain_and_compression_are_saved_native_film_effects() {
        let f: FilmEffects = serde_json::from_value(serde_json::json!({
            "grain_amount": 0.4, "grain_seed": 17,
            "compression_enabled": true, "compression_impact": 0.62,
            "compression_range": 0.43, "compression_white_point": 1.08,
            "compression_color_density": 0.27
        }))
        .unwrap();
        f.validate().unwrap();
        assert!(f.grain_active() && f.compression_active());
        let encoded = serde_json::to_value(f).unwrap();
        assert_eq!(encoded["grain_seed"], 17);
        assert_eq!(f, serde_json::from_value(encoded).unwrap());
        let off = FilmEffects {
            enabled: false,
            ..f
        };
        assert!(!off.active());
    }
    #[test]
    fn older_settings_default_to_bypass_and_profiles_validate() {
        let f: FilmEffects = serde_json::from_str("{}").unwrap();
        assert!(!f.active());
        for p in FilmFormat::ALL {
            assert!(FilmEffects::preset(p).validate().is_ok());
        }
        let mut f = f;
        f.bloom_amount = f32::NAN;
        assert!(f.validate().is_err());
    }
    #[test]
    fn new_recipe_bypasses_every_operator_and_rejects_unknown_controls() {
        let f = FilmEffects::default();
        assert!(f.enabled && f.halation_enabled && f.bloom_enabled && f.grain_enabled);
        assert!(!f.halation_active() && !f.bloom_active());
        assert!(!f.grain_active() && !f.compression_active());
        assert_eq!(f.grain_seed, 42);
        let error =
            serde_json::from_str::<FilmEffects>(r#"{"experimental_texture":0.4}"#).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
    #[test]
    fn format_starting_points_progress_from_fine_to_broad() {
        let profiles = [
            FilmFormat::Large65,
            FilmFormat::Standard35,
            FilmFormat::Small16,
            FilmFormat::Small8,
        ]
        .map(FilmEffects::preset);
        for pair in profiles.windows(2) {
            assert!(pair[1].halation_spread > pair[0].halation_spread);
            assert!(pair[1].halation_amount > pair[0].halation_amount);
            assert!(pair[1].bloom_spread > pair[0].bloom_spread);
            assert!(pair[1].bloom_amount > pair[0].bloom_amount);
        }
        for (regular, open) in [
            (FilmFormat::Small8, FilmFormat::Small8NoRemjet),
            (FilmFormat::Small16, FilmFormat::Small16NoRemjet),
            (FilmFormat::Standard35, FilmFormat::Strong35),
            (FilmFormat::Large65, FilmFormat::Large65NoRemjet),
        ] {
            let (regular, open) = (FilmEffects::preset(regular), FilmEffects::preset(open));
            assert!(open.halation_spread > regular.halation_spread);
            assert!(open.halation_source_limiter < regular.halation_source_limiter);
            assert_eq!(open.bloom_values(), regular.bloom_values());
        }
    }
    #[test]
    fn switches_preserve_amounts_and_older_film_settings_stay_enabled() {
        let mut f: FilmEffects =
            serde_json::from_str(r#"{"halation_amount":0.7,"bloom_amount":0.5}"#).unwrap();
        assert!(f.halation_active() && f.bloom_active());
        f.halation_enabled = false;
        assert!(!f.halation_active() && f.bloom_active());
        f.bloom_enabled = false;
        assert!(!f.active());
        let back: FilmEffects = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(back, f);
        f.halation_enabled = true;
        f.bloom_enabled = true;
        assert_eq!(f.halation_amount, 0.7);
        assert_eq!(f.bloom_amount, 0.5);
        assert!(!serde_json::to_string(&f).unwrap().contains("enabled"));
    }
    #[test]
    fn preset_amount_takes_the_target_switches_above_zero() {
        let from = FilmEffects::default();
        let to = FilmEffects {
            halation_enabled: false,
            bloom_amount: 0.8,
            ..from
        };
        assert!(from.interpolate(to, 0.).halation_enabled);
        let halfway = from.interpolate(to, 0.5);
        assert!(!halfway.halation_enabled);
        assert_eq!(halfway.bloom_amount, 0.4);
    }
}

#[cfg(test)]
mod section_tests {
    use super::*;
    #[test]
    fn expanded_film_controls_round_trip_without_losing_older_values() {
        let old = FilmEffects::preset(FilmFormat::Strong35);
        let mut saved = serde_json::to_value(old).unwrap();
        for key in [
            "halation_source_limiter",
            "halation_background_gain",
            "halation_smoothness",
            "halation_global_diffusion",
            "halation_amplify",
            "halation_blue_compensation",
            "bloom_source_limiter",
            "bloom_details",
            "bloom_amplify",
        ] {
            saved[key] = serde_json::to_value(0.91f32).unwrap();
        }
        let expanded: FilmEffects = serde_json::from_value(saved.clone()).unwrap();
        expanded.validate().unwrap();
        assert_eq!(serde_json::to_value(expanded).unwrap(), saved);
        assert_eq!(expanded.halation_amount, old.halation_amount);
        assert_eq!(expanded.halation_spread, old.halation_spread);
        assert_eq!(expanded.bloom_threshold, old.bloom_threshold);
    }
    #[test]
    fn independent_format_profiles_preserve_the_other_effect_and_master() {
        let mut f = FilmEffects::preset(FilmFormat::Standard35);
        f.enabled = false;
        let bloom = f.bloom_values();
        f.apply_halation_preset(FilmFormat::Strong35);
        assert_eq!(f.bloom_values(), bloom);
        assert!(!f.enabled);
        assert_eq!(f.halation_format(), Some(FilmFormat::Strong35));
        let halation = f.halation_values();
        f.apply_bloom_preset(FilmFormat::Small8);
        assert_eq!(f.halation_values(), halation);
        assert_eq!(f.bloom_format(), Some(FilmFormat::Small8));
        assert!(!f.enabled);
    }
    #[test]
    fn film_section_master_bypasses_both_without_losing_settings() {
        let before = FilmEffects::preset(FilmFormat::Strong35);
        let mut saved = serde_json::to_value(before).unwrap();
        saved["enabled"] = false.into();
        let disabled: FilmEffects = serde_json::from_value(saved).unwrap();
        assert!(!disabled.active());
        assert!(!disabled.halation_active());
        assert!(!disabled.bloom_active());
        assert_eq!(disabled.values(), before.values());
        assert_eq!(disabled.halation_enabled, before.halation_enabled);
        assert_eq!(disabled.bloom_enabled, before.bloom_enabled);
    }
}
