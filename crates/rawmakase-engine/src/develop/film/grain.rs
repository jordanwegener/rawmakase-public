//! Original coordinate-seeded grain with an explicit native-pixel scale.
//!
//! The random field is newly authored smooth lattice noise. Preview filtering
//! uses a statistical area-average model, not a stored texture or output table.

use std::sync::atomic::AtomicBool;

use anyhow::{Result, ensure};
use rayon::prelude::*;

use crate::model::film::FilmEffects;

const BLUR_PASSES: u32 = 3;

fn unit(value: f32, fallback: f64) -> f64 {
    if value.is_finite() {
        f64::from(value).clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn softness_radius(f: FilmEffects, full: [u32; 2]) -> u32 {
    let softness = 1.0 - unit(f.grain_resolution, 1.0);
    // Source softness follows image height/width, hence preview dimensions
    // reduce its support too. The cap bounds work for unusual huge metadata.
    (f64::from(full[0].min(full[1])) * 0.0015 * softness)
        .ceil()
        .clamp(0.0, 32.0) as u32
}

pub(super) fn halo(f: FilmEffects, full: [u32; 2]) -> u32 {
    if !f.grain_active() {
        0
    } else {
        softness_radius(f, full) * BLUR_PASSES
    }
}

// Standard integer avalanche operations give reproducible random lattice
// samples. Integer wraparound is intentional; no global RNG state is used.
fn lattice(x: i64, y: i64, seed: u64, stream: u64) -> f64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (y as u64).wrapping_mul(0xd1b5_4a32_d192_ed03)
        ^ stream.wrapping_mul(0x94d0_49bb_1331_11eb);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    // Fifty-three bits, converted exactly to a centered uniform variate.
    ((h >> 11) as f64 / 9_007_199_254_740_992.0 - 0.5) * 3.464_101_615_137_754_4
}

fn smooth(t: f64) -> f64 {
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

fn field(x: f64, y: f64, seed: u64, stream: u64) -> f64 {
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let u = smooth(x - ix as f64);
    let v = smooth(y - iy as f64);
    let a = lattice(ix, iy, seed, stream);
    let b = lattice(ix.saturating_add(1), iy, seed, stream);
    let c = lattice(ix, iy.saturating_add(1), seed, stream);
    let d = lattice(ix.saturating_add(1), iy.saturating_add(1), seed, stream);
    let top = a + (b - a) * u;
    let bottom = c + (d - c) * u;
    top + (bottom - top) * v
}

fn band_attenuation(width: f64, scale: [f64; 2]) -> f64 {
    // For quintic interpolation, integral((1-s)^2+s^2) = 181/231.
    // Its squared value is the 2D point variance; the covariance integral
    // is width^2 because interpolation weights form a partition of unity.
    // Thus A=width^2/point_variance is the correlation area in native pixels.
    // Approximate a square footprint of N pixels with effective count
    // 1+(N-1)/A. A>=1 retains the independent native-pixel noise limit.
    let point_variance = (181.0_f64 / 231.0).powi(2);
    let area = (width * width / point_variance).max(1.0);
    if scale[0] == scale[1] {
        // Preserve the isotropic formula and its original rounding.
        let s = scale[0];
        return (area * s * s / (1.0 + (area - 1.0) * s * s)).sqrt();
    }
    // An anisotropic footprint consists of its short-axis square plus a
    // remaining strip. The strip's correlation area is L*min(L,short),
    // where L=sqrt(A); a one-row footprint therefore averages in only one
    // dimension. This is continuous with the square model and gives RMS
    // sqrt(scale_x) for independent grain when scale_y=1, rather than
    // incorrectly attenuating for nonexistent rows in a thin photograph.
    let nx = 1.0 / scale[0];
    let ny = 1.0 / scale[1];
    let short = nx.min(ny);
    let long = nx.max(ny);
    let length = area.sqrt();
    let square = (short * short - 1.0) / area;
    let strip = (long - short) * short / (length * length.min(short));
    (1.0 + square + strip).sqrt().recip()
}

fn noise(x: f64, y: f64, width: f64, scale: f64, seed: u64, stream: u64) -> f64 {
    let coarse = field(x / width, y / width, seed, stream);
    let fine = field(
        2.03 * x / width + 13.7,
        2.03 * y / width - 7.9,
        seed,
        stream.wrapping_add(1),
    );
    // Two independent bands avoid a single regular clump scale. Their
    // attenuation is independent because their correlation areas differ.
    1.35 * (0.86 * coarse * band_attenuation(width, [scale; 2])
        + 0.30 * fine * band_attenuation(width / 2.03, [scale; 2]))
}

fn rgb_noise(x: f64, y: f64, width: f64, scale: f64, seed: u64, color: f64) -> [f64; 3] {
    let neutral = noise(x, y, width, scale, seed, 0);
    let mut delta = [neutral; 3];
    if color > 0.0 {
        let u = noise(x, y, width, scale, seed, 16);
        let v = noise(x, y, width, scale, seed, 32);
        let red = 0.60 * color * (u - 0.25 * v);
        let blue = 0.60 * color * (v - 0.25 * u);
        // Two opponent axes have zero luminance, so Color=0 is exactly
        // neutral and adding color preserves luminance RMS.
        delta[0] += red;
        delta[1] -= (0.2126 * red + 0.0722 * blue) / 0.7152;
        delta[2] += blue;
    }
    delta
}

fn opponent_noise(neutral: f64, u: f64, v: f64, color: f64) -> [f64; 3] {
    let red = 0.60 * color * (u - 0.25 * v);
    let blue = 0.60 * color * (v - 0.25 * u);
    [
        neutral + red,
        neutral - (0.2126 * red + 0.0722 * blue) / 0.7152,
        neutral + blue,
    ]
}

fn band_rgb(x: f64, y: f64, seed: u64, stream: u64, color: f64) -> [f64; 3] {
    let neutral = field(x, y, seed, stream);
    if color == 0.0 {
        [neutral; 3]
    } else {
        opponent_noise(
            neutral,
            field(x, y, seed, stream + 16),
            field(x, y, seed, stream + 32),
            color,
        )
    }
}

struct AxisWeights {
    first: i64,
    values: Vec<f64>,
}

fn lanczos3(x: f64) -> f64 {
    let x = x.abs();
    if x >= 3.0 {
        0.0
    } else if x < 1e-12 {
        1.0
    } else {
        let t = std::f64::consts::PI * x;
        (t.sin() / t) * ((t / 3.0).sin() / (t / 3.0))
    }
}

fn axis_weights(position: u32, full: u32, scale: f64, width: f64, phase: f64) -> AxisWeights {
    let center = (f64::from(position) + 0.5) / scale;
    let native_extent = (f64::from(full) / scale).round().max(1.0);
    let left = (center - 3.0 / scale).floor().max(0.0) as i64;
    let right = (center + 3.0 / scale).ceil().min(native_extent) as i64;
    let first = (((left as f64 + 0.5) / width) + phase).floor() as i64;
    let last = (((right as f64 - 0.5) / width) + phase).floor() as i64 + 1;
    let mut values = vec![0.0; (last - first + 1) as usize];
    let mut total = 0.0;
    // Project the native-pixel resampling kernel onto lattice coefficients.
    // field(n+.5) is linear in its two quintic lattice weights per axis;
    // separability lets us collapse all native samples before any hashing.
    for native in left..right {
        let sample = native as f64 + 0.5;
        let weight = lanczos3((sample - center) * scale);
        let coordinate = sample / width + phase;
        let cell = coordinate.floor() as i64;
        let t = smooth(coordinate - cell as f64);
        let index = (cell - first) as usize;
        values[index] += weight * (1.0 - t);
        values[index + 1] += weight * t;
        total += weight;
    }
    for value in &mut values {
        *value /= total;
    }
    AxisWeights { first, values }
}

struct ExactBand {
    horizontal: Vec<AxisWeights>,
    vertical: Vec<AxisWeights>,
    blend: f64,
}

#[derive(Clone, Copy)]
struct PreviewGrid {
    size: [u32; 2],
    full: [u32; 2],
    origin: [u32; 2],
    scale: [f64; 2],
}

fn exact_band(
    grid: PreviewGrid,
    width: f64,
    phase: [f64; 2],
    cancel: &AtomicBool,
) -> Result<Option<ExactBand>> {
    let output_width = width * grid.scale[0].min(grid.scale[1]);
    if grid.scale == [1.0; 2] || output_width <= 0.25 {
        return Ok(None);
    }
    // Below a quarter output pixel the field is handled statistically.
    // Above half a pixel its actual native footprint is affordable to
    // integrate. The transition bounds each axis to a small fixed number
    // of lattice coefficients, independent of photo size or reduction.
    let blend = smooth(((output_width - 0.25) / 0.25).clamp(0.0, 1.0));
    let mut axes = [Vec::new(), Vec::new()];
    for (axis, values) in axes.iter_mut().enumerate() {
        for position in 0..grid.size[axis] {
            super::check(cancel)?;
            values.push(axis_weights(
                grid.origin[axis] + position,
                grid.full[axis],
                grid.scale[axis],
                width,
                phase[axis],
            ));
        }
    }
    let [horizontal, vertical] = axes;
    Ok(Some(ExactBand {
        horizontal,
        vertical,
        blend,
    }))
}

fn exact_rgb(x: &AxisWeights, y: &AxisWeights, seed: u64, stream: u64, color: f64) -> [f64; 3] {
    let mut values = [0.0; 3];
    for (iy, wy) in y.values.iter().copied().enumerate() {
        for (ix, wx) in x.values.iter().copied().enumerate() {
            let px = x.first + ix as i64;
            let py = y.first + iy as i64;
            let weight = wx * wy;
            values[0] += weight * lattice(px, py, seed, stream);
            if color > 0.0 {
                values[1] += weight * lattice(px, py, seed, stream + 16);
                values[2] += weight * lattice(px, py, seed, stream + 32);
            }
        }
    }
    opponent_noise(values[0], values[1], values[2], color)
}

fn finite(value: f32) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        f64::from(value).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
    }
}

fn store(value: f64) -> f32 {
    value.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
}

pub(super) fn apply(
    pixels: &mut [[f32; 3]],
    size: [u32; 2],
    full: [u32; 2],
    origin: [u32; 2],
    f: FilmEffects,
    output_scale: [f32; 2],
    cancel: &AtomicBool,
) -> Result<()> {
    if !f.grain_active() {
        return Ok(());
    }
    super::check(cancel)?;
    ensure!(
        u64::from(size[0]) * u64::from(size[1]) == pixels.len() as u64,
        "grain dimensions do not match the pixel buffer"
    );
    ensure!(
        u64::from(origin[0]) + u64::from(size[0]) <= u64::from(full[0])
            && u64::from(origin[1]) + u64::from(size[1]) <= u64::from(full[1]),
        "grain region lies outside the full output"
    );
    if pixels.is_empty() {
        return Ok(());
    }

    let amount = unit(f.grain_amount, 0.0);
    if amount == 0.0 {
        return Ok(());
    }
    let size_control = unit(f.grain_size, 0.5);
    let grain_width = 0.65 + 7.35 * size_control * size_control;
    let color = unit(f.grain_color, 0.0);
    let softness = 1.0 - unit(f.grain_resolution, 1.0);
    let shadows = unit(f.grain_shadows, 0.5);
    let midtones = unit(f.grain_midtones, 0.5);
    let highlights = unit(f.grain_highlights, 0.5);
    // Production previews and exports use 0 < scale <= 1. Invalid values
    // fall back to native scale; an extremely small positive scale is still
    // supported without allocating or traversing a native-sized canvas.
    let scale = output_scale.map(|value| {
        if value.is_finite() && value > 0.0 {
            f64::from(value).clamp(1e-8, 1.0)
        } else {
            1.0
        }
    });
    let preview = scale != [1.0; 2];
    let grid = PreviewGrid {
        size,
        full,
        origin,
        scale,
    };
    let exact = [
        exact_band(grid, grain_width, [0.0, 0.0], cancel)?,
        exact_band(grid, grain_width / 2.03, [13.7, -7.9], cancel)?,
    ];
    // RMS attenuation alone cannot remove the high-frequency aliases of a
    // sampled noise field. A small symmetric low-pass kernel supplies the
    // neighboring-pixel covariance of resampled native grain. Its strength
    // tends continuously to zero at native scale. Cache the pure RGB field
    // once over this output region plus one pixel, not over the whole photo.
    // Clamp coordinates at whole-output edges so tile edges never change it.
    let preview_noise = if preview
        && exact
            .iter()
            .any(|band| band.as_ref().is_none_or(|b| b.blend < 1.0))
    {
        let cache_width = size[0] as usize + 2;
        let cache_height = size[1] as usize + 2;
        let cache_len = cache_width
            .checked_mul(cache_height)
            .ok_or_else(|| anyhow::anyhow!("grain preview noise dimensions overflow"))?;
        let mut cache = vec![[0.0_f32; 6]; cache_len];
        cache
            .par_chunks_mut(cache_width)
            .enumerate()
            .try_for_each(|(y, row)| -> Result<()> {
                super::check(cancel)?;
                let gy = (i64::from(origin[1]) + y as i64 - 1).clamp(0, i64::from(full[1]) - 1);
                let py = (gy as f64 + 0.5) / scale[1];
                for (x, value) in row.iter_mut().enumerate() {
                    if x % 4096 == 0 {
                        super::check(cancel)?;
                    }
                    let gx = (i64::from(origin[0]) + x as i64 - 1).clamp(0, i64::from(full[0]) - 1);
                    let px = (gx as f64 + 0.5) / scale[0];
                    let coarse =
                        band_rgb(px / grain_width, py / grain_width, f.grain_seed, 0, color);
                    let fine = band_rgb(
                        2.03 * px / grain_width + 13.7,
                        2.03 * py / grain_width - 7.9,
                        f.grain_seed,
                        1,
                        color,
                    );
                    for (c, (coarse, fine)) in coarse.into_iter().zip(fine).enumerate() {
                        value[c] = coarse as f32;
                        value[c + 3] = fine as f32;
                    }
                }
                Ok(())
            })?;
        Some((cache_width, cache))
    } else {
        None
    };
    let weights = scale.map(|s| {
        let side = 0.055 * (1.0 - s).sqrt();
        [side, 1.0 - 2.0 * side, side]
    });
    let attenuations = [
        band_attenuation(grain_width, scale),
        band_attenuation(grain_width / 2.03, scale),
    ];
    let radius = softness_radius(f, full);
    let softened = if radius > 0 {
        let mut source = pixels.to_vec();
        source
            .par_chunks_mut(4096)
            .try_for_each(|chunk| -> Result<()> {
                super::check(cancel)?;
                for pixel in chunk {
                    *pixel = pixel.map(|v| store(finite(v)));
                }
                Ok(())
            })?;
        super::blur(&mut source, size, radius, BLUR_PASSES, cancel)?;
        Some(source)
    } else {
        None
    };
    let source_mix = amount * softness;
    pixels
        .par_chunks_mut(size[0] as usize)
        .enumerate()
        .try_for_each(|(y, row)| -> Result<()> {
            super::check(cancel)?;
            for (x, pixel) in row.iter_mut().enumerate() {
                if x % 4096 == 0 {
                    super::check(cancel)?;
                }
                let index = y * size[0] as usize + x;
                let original = pixel.map(finite);
                let source = if let Some(ref blurred) = softened {
                    let soft = blurred[index].map(finite);
                    // Soften luminance detail, preserving the input's opponent
                    // chroma even when Color=0 and Resolution is low.
                    let delta = source_mix
                        * (0.2126 * (soft[0] - original[0])
                            + 0.7152 * (soft[1] - original[1])
                            + 0.0722 * (soft[2] - original[2]));
                    original.map(|v| v + delta)
                } else {
                    original
                };
                let luma =
                    (0.2126 * source[0] + 0.7152 * source[1] + 0.0722 * source[2]).clamp(0.0, 1.0);
                // Quadratic Bernstein weights form a continuous partition of
                // unity across shadows, midtones and highlights.
                let tonal = shadows * (1.0 - luma).powi(2)
                    + midtones * 2.0 * luma * (1.0 - luma)
                    + highlights * luma * luma;
                let amplitude = 0.065 * amount * tonal;
                let delta = if preview {
                    let mut bands = [[0.0; 3]; 2];
                    if let Some((cache_width, cache)) = &preview_noise {
                        for (dy, wy) in weights[1].into_iter().enumerate() {
                            for (dx, wx) in weights[0].into_iter().enumerate() {
                                let value = cache[(y + dy) * *cache_width + x + dx];
                                for (band, output) in bands.iter_mut().enumerate() {
                                    for (c, channel) in output.iter_mut().enumerate() {
                                        *channel += f64::from(value[band * 3 + c])
                                            * wy
                                            * wx
                                            * attenuations[band];
                                    }
                                }
                            }
                        }
                    }
                    for (band, output) in bands.iter_mut().enumerate() {
                        if let Some(exact) = &exact[band] {
                            let integrated = exact_rgb(
                                &exact.horizontal[x],
                                &exact.vertical[y],
                                f.grain_seed,
                                band as u64,
                                color,
                            );
                            for (channel, sample) in output.iter_mut().zip(integrated) {
                                *channel = (1.0 - exact.blend) * *channel + exact.blend * sample;
                            }
                        }
                    }
                    std::array::from_fn(|c| 1.35 * (0.86 * bands[0][c] + 0.30 * bands[1][c]))
                } else {
                    let px = f64::from(origin[0]) + x as f64 + 0.5;
                    let py = f64::from(origin[1]) + y as f64 + 0.5;
                    rgb_noise(px, py, grain_width, 1.0, f.grain_seed, color)
                };
                *pixel = std::array::from_fn(|c| store(source[c] + amplitude * delta[c]));
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
            grain_enabled: true,
            grain_amount: 0.6,
            grain_size: 0.45,
            grain_shadows: 0.6,
            grain_midtones: 0.7,
            grain_highlights: 0.5,
            grain_color: 0.0,
            grain_resolution: 1.0,
            grain_seed: 71,
            ..FilmEffects::default()
        }
    }

    fn run(source: &[[f32; 3]], size: [u32; 2], f: FilmEffects, scale: f32) -> Vec<[f32; 3]> {
        let mut out = source.to_vec();
        apply(
            &mut out,
            size,
            size,
            [0, 0],
            f,
            [scale; 2],
            &AtomicBool::new(false),
        )
        .unwrap();
        out
    }

    #[test]
    fn zero_amount_is_bit_exact_and_neutral_grain_preserves_chroma() {
        let source = vec![[0.2, 0.4, 0.7]; 256];
        let mut off = source.clone();
        off[0] = [-0.0, 0.0, f32::from_bits(0x7fc0_0023)];
        let bypass = run(
            &off,
            [16, 16],
            FilmEffects {
                grain_amount: 0.0,
                ..enabled()
            },
            1.0,
        );
        assert!(
            off.iter()
                .flatten()
                .zip(bypass.iter().flatten())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        let out = run(&source, [16, 16], enabled(), 1.0);
        assert_ne!(out, source);
        for p in out {
            assert!(((p[1] - p[0]) - 0.2).abs() < 1e-7);
            assert!(((p[2] - p[0]) - 0.5).abs() < 1e-7);
        }
        let detailed: Vec<_> = (0..256)
            .map(|i| {
                let v = if i % 2 == 0 { 0.1 } else { 0.4 };
                [v, v + 0.2, v + 0.5]
            })
            .collect();
        let softened = run(
            &detailed,
            [16, 16],
            FilmEffects {
                grain_resolution: 0.0,
                ..enabled()
            },
            1.0,
        );
        assert_ne!(softened, detailed);
        for p in softened {
            assert!(((p[1] - p[0]) - 0.2).abs() < 1.5e-7);
            assert!(((p[2] - p[0]) - 0.5).abs() < 1.5e-7);
        }
    }

    #[test]
    fn seed_and_each_control_have_an_observable_effect() {
        let size = [80, 40];
        let source: Vec<_> = (0..3200)
            .map(|i| {
                let v = (i % 80) as f32 / 79.0;
                [v, v * 0.8, v * 0.6]
            })
            .collect();
        let baseline = run(&source, size, enabled(), 1.0);
        assert_eq!(baseline, run(&source, size, enabled(), 1.0));
        for f in [
            FilmEffects {
                grain_seed: 72,
                ..enabled()
            },
            FilmEffects {
                grain_amount: 0.2,
                ..enabled()
            },
            FilmEffects {
                grain_size: 0.8,
                ..enabled()
            },
            FilmEffects {
                grain_shadows: 0.1,
                ..enabled()
            },
            FilmEffects {
                grain_midtones: 0.1,
                ..enabled()
            },
            FilmEffects {
                grain_highlights: 0.1,
                ..enabled()
            },
            FilmEffects {
                grain_color: 0.8,
                ..enabled()
            },
            FilmEffects {
                grain_resolution: 0.0,
                ..enabled()
            },
        ] {
            assert_ne!(baseline, run(&source, size, f, 1.0));
        }
    }

    #[test]
    fn crop_with_halo_matches_full_at_native_and_preview_scale() {
        let full = [96, 72];
        let source: Vec<_> = (0..full[0] * full[1])
            .map(|i| {
                let x = i % full[0];
                let y = i / full[0];
                [x as f32 / 95.0, y as f32 / 71.0, (x * y % 43) as f32 / 42.0]
            })
            .collect();
        let f = FilmEffects {
            grain_resolution: 0.0,
            grain_color: 0.7,
            ..enabled()
        };
        let border = halo(f, full);
        let interior_origin = [21, 17];
        let interior_size = [31, 25];
        let origin = [interior_origin[0] - border, interior_origin[1] - border];
        let size = [interior_size[0] + 2 * border, interior_size[1] + 2 * border];
        for scale in [1.0, 0.25] {
            let all = run(&source, full, f, scale);
            let mut crop = Vec::new();
            for y in origin[1]..origin[1] + size[1] {
                let start = (y * full[0] + origin[0]) as usize;
                crop.extend_from_slice(&source[start..start + size[0] as usize]);
            }
            apply(
                &mut crop,
                size,
                full,
                origin,
                f,
                [scale; 2],
                &AtomicBool::new(false),
            )
            .unwrap();
            for y in 0..interior_size[1] {
                for x in 0..interior_size[0] {
                    let a =
                        all[((interior_origin[1] + y) * full[0] + interior_origin[0] + x) as usize];
                    let b = crop[((y + border) * size[0] + x + border) as usize];
                    assert!(a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 4e-6));
                }
            }
        }
    }

    #[test]
    fn preview_noise_filter_matches_edge_crops_without_source_halo() {
        let full = [41, 31];
        let source = vec![[0.5; 3]; (full[0] * full[1]) as usize];
        let size = [7, 6];
        for color in [0.0, 0.7] {
            let f = FilmEffects {
                grain_color: color,
                grain_resolution: 1.0,
                ..enabled()
            };
            assert_eq!(halo(f, full), 0);
            for scale in [0.75, 0.125] {
                let all = run(&source, full, f, scale);
                for origin in [[0, 0], [11, 8], [34, 25]] {
                    let mut crop = vec![[0.5; 3]; (size[0] * size[1]) as usize];
                    apply(
                        &mut crop,
                        size,
                        full,
                        origin,
                        f,
                        [scale; 2],
                        &AtomicBool::new(false),
                    )
                    .unwrap();
                    for y in 0..size[1] {
                        for x in 0..size[0] {
                            let a = all[((origin[1] + y) * full[0] + origin[0] + x) as usize];
                            let b = crop[(y * size[0] + x) as usize];
                            assert!(a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 4e-6));
                            if color == 0.0 {
                                assert_eq!(b[0], b[1]);
                                assert_eq!(b[1], b[2]);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lattice_projection_matches_direct_native_pixel_reduction() {
        let scale = 0.125;
        let full = [16, 12];
        for (width, phase, stream) in [(8.0, [0.0, 0.0], 0), (8.0 / 2.03, [13.7, -7.9], 1)] {
            for position in [[0, 0], [5, 4], [15, 11]] {
                let horizontal = axis_weights(position[0], full[0], scale, width, phase[0]);
                let vertical = axis_weights(position[1], full[1], scale, width, phase[1]);
                let projected = exact_rgb(&horizontal, &vertical, 71, stream, 0.0)[0];
                let center = [
                    (f64::from(position[0]) + 0.5) / scale,
                    (f64::from(position[1]) + 0.5) / scale,
                ];
                let mut direct = 0.0;
                let mut total = 0.0;
                // Independently reduce the actual synthetic native pixel
                // field; this deliberately does not use collapsed weights.
                for y in 0..96 {
                    let wy = lanczos3((f64::from(y) + 0.5 - center[1]) * scale);
                    for x in 0..128 {
                        let wx = lanczos3((f64::from(x) + 0.5 - center[0]) * scale);
                        let weight = wx * wy;
                        direct += weight
                            * field(
                                (f64::from(x) + 0.5) / width + phase[0],
                                (f64::from(y) + 0.5) / width + phase[1],
                                71,
                                stream,
                            );
                        total += weight;
                    }
                }
                assert!((projected - direct / total).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn thin_previews_keep_the_native_short_axis_and_crop_coordinates() {
        for (native_size, output_size, scale, origin, crop_size) in [
            ([1024, 1], [128, 1], [0.125, 1.0], [31, 0], [57, 1]),
            ([1, 1024], [1, 128], [1.0, 0.125], [0, 31], [1, 57]),
        ] {
            for grain_size in [0.0, 1.0] {
                let f = FilmEffects {
                    grain_size,
                    ..enabled()
                };
                let source = vec![[0.5; 3]; 1024];
                let native = run(&source, native_size, f, 1.0);
                let mut preview = vec![[0.5; 3]; 128];
                apply(
                    &mut preview,
                    output_size,
                    output_size,
                    [0, 0],
                    f,
                    scale,
                    &AtomicBool::new(false),
                )
                .unwrap();
                if grain_size == 0.0 {
                    // One-dimensional reduction averages eight samples,
                    // rather than an eight-by-eight square of nonexistent
                    // native rows. The loose statistical range tolerates
                    // the finite seeded sample while rejecting that error.
                    let ratio = rms(&preview) / rms(&native);
                    assert!(ratio > 0.24 && ratio < 0.48, "thin RMS ratio {ratio}");
                }
                let mut crop = vec![[0.5; 3]; 57];
                apply(
                    &mut crop,
                    crop_size,
                    output_size,
                    origin,
                    f,
                    scale,
                    &AtomicBool::new(false),
                )
                .unwrap();
                for (index, pixel) in crop.iter().enumerate() {
                    assert_eq!(pixel[0], pixel[1]);
                    assert_eq!(pixel[1], pixel[2]);
                    assert!(
                        pixel
                            .iter()
                            .zip(preview[31 + index])
                            .all(|(a, b)| (*a - b).abs() < 4e-6)
                    );
                }
            }
        }
    }

    #[test]
    fn anisotropic_lattice_projection_respects_finite_native_rows() {
        let scale = [0.125, 1.0 / 3.0];
        let horizontal = axis_weights(5, 16, scale[0], 8.0, 0.0);
        let vertical = axis_weights(0, 1, scale[1], 8.0, 0.0);
        let projected = exact_rgb(&horizontal, &vertical, 71, 0, 0.0)[0];
        let center = [5.5 / scale[0], 0.5 / scale[1]];
        let mut direct = 0.0;
        let mut total = 0.0;
        for y in 0..3 {
            for x in 0..128 {
                let weight = lanczos3((f64::from(x) + 0.5 - center[0]) * scale[0])
                    * lanczos3((f64::from(y) + 0.5 - center[1]) * scale[1]);
                direct += weight
                    * field(
                        (f64::from(x) + 0.5) / 8.0,
                        (f64::from(y) + 0.5) / 8.0,
                        71,
                        0,
                    );
                total += weight;
            }
        }
        assert!((projected - direct / total).abs() < 1e-10);
        assert!((band_attenuation(0.65, [0.125, 1.0]) - 0.125_f64.sqrt()).abs() < 1e-12);
        assert!((band_attenuation(0.65, [1.0, 0.125]) - 0.125_f64.sqrt()).abs() < 1e-12);
    }

    fn rms(pixels: &[[f32; 3]]) -> f64 {
        let mean = pixels.iter().map(|p| f64::from(p[0]) - 0.5).sum::<f64>() / pixels.len() as f64;
        (pixels
            .iter()
            .map(|p| (f64::from(p[0]) - 0.5 - mean).powi(2))
            .sum::<f64>()
            / pixels.len() as f64)
            .sqrt()
    }

    #[test]
    fn preview_reduces_fine_grain_rms_and_clump_width() {
        let size = [192, 128];
        let source = vec![[0.5; 3]; (size[0] * size[1]) as usize];
        let f = FilmEffects {
            grain_size: 0.0,
            ..enabled()
        };
        let native = run(&source, size, f, 1.0);
        let preview = run(&source, size, f, 0.125);
        let ratio = rms(&preview) / rms(&native);
        assert!(ratio > 0.10 && ratio < 0.35, "RMS ratio {ratio}");

        let f = FilmEffects {
            grain_size: 1.0,
            ..enabled()
        };
        let native = run(&source, size, f, 1.0);
        let preview = run(&source, size, f, 0.25);
        let roughness = |pixels: &[[f32; 3]]| {
            let differences = pixels
                .chunks_exact(size[0] as usize)
                .flat_map(|row| row.windows(2).map(|p| f64::from(p[1][0] - p[0][0]).powi(2)))
                .sum::<f64>();
            differences / pixels.len() as f64 / rms(pixels).powi(2)
        };
        assert!(roughness(&preview) > 2.0 * roughness(&native));
    }

    #[test]
    fn active_grain_is_finite_for_extreme_samples_and_controls() {
        let source = vec![[f32::MAX, -f32::MAX, f32::NAN]; 64];
        let f = FilmEffects {
            grain_amount: 1.0,
            grain_color: 1.0,
            grain_resolution: 1.0,
            grain_size: f32::NAN,
            ..enabled()
        };
        for scale in [f32::NAN, 0.0, 1e-20, 1.0] {
            assert!(
                run(&source, [8, 8], f, scale)
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite())
            );
        }
    }
}
