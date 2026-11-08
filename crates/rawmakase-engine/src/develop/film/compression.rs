//! An original analytic highlight shoulder, using no response tables.

use std::sync::atomic::AtomicBool;

use anyhow::Result;
use rayon::prelude::*;

use crate::model::film::FilmEffects;

fn unit(value: f32, fallback: f64) -> f64 {
    if value.is_finite() {
        f64::from(value).clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn finite_channel(value: f32) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        f64::from(value).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
    }
}

fn store(value: f64) -> f32 {
    value.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
}

pub(super) fn apply(pixels: &mut [[f32; 3]], f: FilmEffects, cancel: &AtomicBool) -> Result<()> {
    // In particular, disabled processing preserves signed zeros and NaN payloads.
    if !f.compression_active() {
        return Ok(());
    }
    super::check(cancel)?;
    let impact = unit(f.compression_impact, 0.0);
    let range = unit(f.compression_range, 0.5);
    let density = unit(f.compression_color_density, 1.0);
    let white = if f.compression_white_point.is_finite() {
        f64::from(f.compression_white_point).clamp(0.5, 1.2)
    } else {
        1.0
    };
    if impact == 0.0 && white == 1.0 {
        return Ok(());
    }

    // Increasing Range lowers the knee from 90% to 12% of scene white.
    // For t = x-k > 0 and d = 1-k, S(x) = k + t/sqrt(1+(t/d)^2).
    // S joins the identity with equal first and second derivatives at k,
    // has strictly positive slope, and approaches one without overshooting.
    // Mixing S with x preserves monotonicity. White Point is an output gain:
    // W*((1-I)*x + I*S(x)), including when I=0.
    let knee = 0.9 - 0.78 * range;
    let headroom = 1.0 - knee;
    pixels
        .par_chunks_mut(4096)
        .try_for_each(|chunk| -> Result<()> {
            super::check(cancel)?;
            for pixel in chunk {
                let rgb = pixel.map(finite_channel);
                let peak = rgb[0].max(rgb[1]).max(rgb[2]).max(0.0);
                let shoulder = if peak > knee {
                    let t = peak - knee;
                    knee + t / (1.0 + (t / headroom).powi(2)).sqrt()
                } else {
                    peak
                };
                // Use the convex form: x + I*(S-x) loses the entire shoulder to
                // cancellation for very bright finite inputs when I=1.
                let compressed = (1.0 - impact) * peak + impact * shoulder;
                let gain = if peak > 0.0 {
                    white * compressed / peak
                } else {
                    white
                };
                let scaled = rgb.map(|v| v * gain);

                // A convex move toward luminance keeps the hue direction and neutral
                // colors. Low density removes chroma only where the shoulder acts;
                // density=1 retains it. Negative channels remain signed, not clipped.
                let compression_fraction = if peak > 0.0 {
                    (1.0 - shoulder / peak).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let saturation = 1.0 - (1.0 - density) * impact * compression_fraction;
                let luma = 0.2126 * scaled[0] + 0.7152 * scaled[1] + 0.0722 * scaled[2];
                *pixel = scaled.map(|v| store((1.0 - saturation) * luma + saturation * v));
            }
            Ok(())
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled() -> FilmEffects {
        FilmEffects {
            enabled: true,
            compression_enabled: true,
            compression_impact: 1.0,
            compression_range: 0.6,
            compression_white_point: 1.0,
            compression_color_density: 1.0,
            ..FilmEffects::default()
        }
    }

    fn run(mut pixels: Vec<[f32; 3]>, f: FilmEffects) -> Vec<[f32; 3]> {
        apply(&mut pixels, f, &AtomicBool::new(false)).unwrap();
        pixels
    }

    #[test]
    fn disabled_is_bit_exact_and_white_point_can_work_alone() {
        let source = vec![[0.0, -0.0, 0.125], [0.5, 1.0, 2.0]];
        let off = FilmEffects {
            compression_impact: 0.0,
            compression_white_point: 1.0,
            ..enabled()
        };
        let out = run(source.clone(), off);
        for (a, b) in source.iter().flatten().zip(out.iter().flatten()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        let out = run(
            source,
            FilmEffects {
                compression_white_point: 0.75,
                ..off
            },
        );
        assert_eq!(out[1], [0.375, 0.75, 1.5]);
    }

    #[test]
    fn neutral_ramp_is_monotonic_and_has_a_smooth_knee() {
        assert!(enabled().compression_active());
        assert!(run(vec![[4.0; 3]], enabled())[0][0] < 1.0);
        for range in [0.0, 0.4, 1.0] {
            for impact in [0.0, 0.25, 1.0] {
                let f = FilmEffects {
                    compression_range: range,
                    compression_impact: impact,
                    ..enabled()
                };
                let source = (0..4096).map(|i| [i as f32 / 256.0; 3]).collect();
                let out = run(source, f);
                assert!(out.windows(2).all(|v| v[0][0] <= v[1][0]));
                assert!(out.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
                let knee = 0.9 - 0.78 * range;
                let step = 0.0005;
                let around = run(vec![[knee - step; 3], [knee; 3], [knee + step; 3]], f);
                let left = (around[1][0] - around[0][0]) / step;
                let right = (around[2][0] - around[1][0]) / step;
                assert!((left - right).abs() < 0.004);
            }
        }
    }

    #[test]
    fn controls_change_highlights_and_density_preserves_neutral_colors() {
        let source = vec![[4.0, 1.0, 0.25], [4.0; 3]];
        let dense = run(source.clone(), enabled());
        let pale = run(
            source.clone(),
            FilmEffects {
                compression_color_density: 0.0,
                ..enabled()
            },
        );
        assert!(dense[0][0] - dense[0][2] > pale[0][0] - pale[0][2]);
        assert_eq!(dense[1], pale[1]);
        assert!((dense[0][0] / dense[0][1] - 4.0).abs() < 1e-5);
        for changed in [
            FilmEffects {
                compression_range: 0.0,
                ..enabled()
            },
            FilmEffects {
                compression_impact: 0.4,
                ..enabled()
            },
            FilmEffects {
                compression_white_point: 0.7,
                ..enabled()
            },
        ] {
            assert_ne!(dense, run(source.clone(), changed));
        }
    }

    #[test]
    fn signed_extremes_and_nonfinite_samples_have_defined_finite_output() {
        let source = vec![
            [f32::MAX; 3],
            [-f32::MAX; 3],
            [f32::MAX, -f32::MAX, 0.0],
            [f32::NAN, f32::INFINITY, f32::NEG_INFINITY],
        ];
        for impact in [0.0, 0.5, 1.0] {
            let out = run(
                source.clone(),
                FilmEffects {
                    compression_impact: impact,
                    compression_white_point: 1.2,
                    compression_color_density: 0.0,
                    ..enabled()
                },
            );
            assert!(out.iter().flatten().all(|v| v.is_finite()));
        }
        let extreme_ramp = run(
            [1.0, 16.0, 1e6, 1e16, 1e30, f32::MAX]
                .into_iter()
                .map(|v| [v; 3])
                .collect(),
            enabled(),
        );
        assert!(extreme_ramp.windows(2).all(|v| v[0][0] <= v[1][0]));
        assert!(extreme_ramp.last().unwrap()[0] > 0.99);
    }
}
