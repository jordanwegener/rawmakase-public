use super::FilmEffects;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrainFormat {
    Small8Iso50,
    Small8Iso250,
    Small8Iso500,
    Small16Iso50,
    Small16Iso250,
    Small16Iso500,
    Standard35Iso50,
    Standard35Iso250,
    Standard35Iso500,
    Large65Iso50,
    Large65Iso250,
    Large65Iso500,
}
impl GrainFormat {
    pub const ALL: [Self; 12] = [
        Self::Small8Iso50,
        Self::Small8Iso250,
        Self::Small8Iso500,
        Self::Small16Iso50,
        Self::Small16Iso250,
        Self::Small16Iso500,
        Self::Standard35Iso50,
        Self::Standard35Iso250,
        Self::Standard35Iso500,
        Self::Large65Iso50,
        Self::Large65Iso250,
        Self::Large65Iso500,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Small8Iso50 => "8 mm / ISO 50",
            Self::Small8Iso250 => "8 mm / ISO 250",
            Self::Small8Iso500 => "8 mm / ISO 500",
            Self::Small16Iso50 => "16 mm / ISO 50",
            Self::Small16Iso250 => "16 mm / ISO 250",
            Self::Small16Iso500 => "16 mm / ISO 500",
            Self::Standard35Iso50 => "35 mm / ISO 50",
            Self::Standard35Iso250 => "35 mm / ISO 250",
            Self::Standard35Iso500 => "35 mm / ISO 500",
            Self::Large65Iso50 => "65 mm / ISO 50",
            Self::Large65Iso250 => "65 mm / ISO 250",
            Self::Large65Iso500 => "65 mm / ISO 500",
        }
    }
    fn settings(self) -> FilmEffects {
        // Coordinates describe an aesthetic progression, not calibrated film.
        let scale = match self {
            Self::Small8Iso50 | Self::Small8Iso250 | Self::Small8Iso500 => 1.,
            Self::Small16Iso50 | Self::Small16Iso250 | Self::Small16Iso500 => 2. / 3.,
            Self::Standard35Iso50 | Self::Standard35Iso250 | Self::Standard35Iso500 => 1. / 3.,
            Self::Large65Iso50 | Self::Large65Iso250 | Self::Large65Iso500 => 0.,
        };
        let speed = match self {
            Self::Small8Iso50 | Self::Small16Iso50 | Self::Standard35Iso50 | Self::Large65Iso50 => {
                0.
            }
            Self::Small8Iso250
            | Self::Small16Iso250
            | Self::Standard35Iso250
            | Self::Large65Iso250 => 0.5,
            Self::Small8Iso500
            | Self::Small16Iso500
            | Self::Standard35Iso500
            | Self::Large65Iso500 => 1.,
        };
        FilmEffects {
            grain_enabled: true,
            grain_size: 0.12 + 0.48 * scale + 0.10 * speed,
            grain_amount: 0.14 + 0.18 * scale + 0.22 * speed,
            grain_shadows: 0.48 + 0.08 * speed,
            grain_midtones: 0.76,
            grain_highlights: 0.28 + 0.08 * speed,
            grain_color: 0.16 + 0.14 * speed,
            grain_resolution: 0.90 - 0.24 * scale - 0.12 * speed,
            ..Default::default()
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionFormat {
    Soft,
    Dense,
    Faded,
}
impl CompressionFormat {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Dense, Self::Faded];
    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft Roll-Off",
            Self::Dense => "Dense Highlights",
            Self::Faded => "Faded Whites",
        }
    }
    fn settings(self) -> FilmEffects {
        // Three original highlight treatments along a common strength axis.
        let strength = match self {
            Self::Soft => 0.28,
            Self::Dense => 0.76,
            Self::Faded => 0.52,
        };
        FilmEffects {
            compression_enabled: true,
            compression_impact: strength,
            compression_range: 0.18 + 0.50 * strength,
            compression_white_point: if self == Self::Faded { 0.91 } else { 1. },
            compression_color_density: if self == Self::Faded {
                0.15
            } else {
                0.22 + 0.50 * strength
            },
            ..Default::default()
        }
    }
}
impl FilmEffects {
    pub fn grain_format(self) -> Option<GrainFormat> {
        GrainFormat::ALL
            .into_iter()
            .find(|v| self.grain_values() == v.settings().grain_values())
    }
    pub fn compression_format(self) -> Option<CompressionFormat> {
        CompressionFormat::ALL
            .into_iter()
            .find(|v| self.compression_values() == v.settings().compression_values())
    }
    pub fn apply_grain_preset(&mut self, preset: GrainFormat) {
        let master = self.enabled;
        let seed = self.grain_seed;
        self.copy_grain_from(preset.settings());
        self.enabled = master;
        self.grain_seed = seed;
    }
    pub fn apply_compression_preset(&mut self, preset: CompressionFormat) {
        let master = self.enabled;
        self.copy_compression_from(preset.settings());
        self.enabled = master;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grain_progresses_with_speed_and_format_size() {
        for formats in GrainFormat::ALL.as_chunks::<3>().0 {
            for pair in formats.windows(2) {
                let (fine, coarse) = (pair[0].settings(), pair[1].settings());
                assert!(coarse.grain_amount > fine.grain_amount);
                assert!(coarse.grain_size > fine.grain_size);
                assert!(coarse.grain_resolution < fine.grain_resolution);
            }
        }
        for speed in 0..3 {
            for smaller_format in 0..3 {
                let small = GrainFormat::ALL[smaller_format * 3 + speed].settings();
                let large = GrainFormat::ALL[(smaller_format + 1) * 3 + speed].settings();
                assert!(small.grain_amount > large.grain_amount);
                assert!(small.grain_size > large.grain_size);
            }
        }
    }
    #[test]
    fn all_profiles_validate_and_change_only_the_selected_effect() {
        let base = FilmEffects {
            enabled: false,
            grain_seed: 713,
            halation_amount: 0.8,
            bloom_amount: 0.6,
            ..Default::default()
        };
        for p in GrainFormat::ALL {
            let mut f = base;
            f.apply_grain_preset(p);
            f.validate().unwrap();
            assert_eq!(f.grain_format(), Some(p));
            assert_eq!(f.grain_seed, 713);
            f.copy_grain_from(base);
            assert_eq!(f, base);
        }
        for p in CompressionFormat::ALL {
            let mut f = base;
            f.apply_compression_preset(p);
            f.validate().unwrap();
            assert_eq!(f.compression_format(), Some(p));
            f.copy_compression_from(base);
            assert_eq!(f, base);
        }
    }
}
