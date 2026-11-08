//! Original public-edition film processing on developed encoded RGB.
//! Spatial diffusion is a finite three-box filter; no external assets or kernels.
use crate::{model::film::FilmEffects, rendered::Rendered};
use anyhow::{Result, ensure};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
mod compression;
mod grain;

/// Rendering revision participates in stored preview identity.
pub fn texture_fingerprint() -> u64 {
    0x7075_626c_6963_0001
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Image,
    Halation,
    Bloom,
}
impl View {
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Halation => "Halation only",
            Self::Bloom => "Bloom only",
        }
    }
}
fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "Render superseded");
    Ok(())
}
fn luminance(p: [f32; 3]) -> f32 {
    p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722
}
fn ramp(value: f32, cutoff: f32) -> f32 {
    if cutoff >= 1. {
        return if value >= cutoff { 1. } else { 0. };
    }
    let t = ((value - cutoff) / (1. - cutoff)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
/// Normalized box average from a prefix sum, extending the endpoint samples.
fn average(
    prefix: &[[f64; 3]],
    first: [f32; 3],
    last: [f32; 3],
    at: usize,
    radius: u32,
) -> [f32; 3] {
    let n = prefix.len() - 1;
    let r = radius as usize;
    let left = at.saturating_sub(r);
    let right = at.saturating_add(r).min(n - 1);
    let before = r.saturating_sub(at) as f64;
    let after = at.saturating_add(r).saturating_sub(n - 1) as f64;
    let count = 2. * radius as f64 + 1.;
    std::array::from_fn(|k| {
        ((prefix[right + 1][k] - prefix[left][k]
            + before * first[k] as f64
            + after * last[k] as f64)
            / count) as f32
    })
}
/// Each pass has radius support on each axis. Prefix sums make work independent
/// of radius, unlike a tap-by-tap convolution. Intermediate sums use f64.
fn blur(
    pixels: &mut [[f32; 3]],
    [w, h]: [u32; 2],
    radius: u32,
    passes: u32,
    cancel: &AtomicBool,
) -> Result<()> {
    if radius == 0 || passes == 0 {
        return check(cancel);
    }
    let (w, h) = (w as usize, h as usize);
    ensure!(
        w > 0 && h > 0 && pixels.len() == w * h,
        "Invalid diffusion dimensions"
    );
    let mut rows = vec![[0.; 3]; pixels.len()];
    let mut columns = vec![[0.; 3]; pixels.len()];
    for _ in 0..passes {
        rows.par_chunks_mut(w)
            .zip(pixels.par_chunks(w))
            .try_for_each(|(out, input)| -> Result<()> {
                check(cancel)?;
                let mut prefix = vec![[0f64; 3]; w + 1];
                for (i, p) in input.iter().enumerate() {
                    for (k, v) in p.iter().enumerate() {
                        prefix[i + 1][k] = prefix[i][k] + *v as f64;
                    }
                }
                for (x, p) in out.iter_mut().enumerate() {
                    *p = average(&prefix, input[0], input[w - 1], x, radius);
                }
                Ok(())
            })?;
        columns
            .par_chunks_mut(h)
            .enumerate()
            .try_for_each(|(x, out)| -> Result<()> {
                check(cancel)?;
                let mut prefix = vec![[0f64; 3]; h + 1];
                for y in 0..h {
                    for k in 0..3 {
                        prefix[y + 1][k] = prefix[y][k] + rows[y * w + x][k] as f64;
                    }
                }
                for (y, p) in out.iter_mut().enumerate() {
                    *p = average(&prefix, rows[x], rows[(h - 1) * w + x], y, radius);
                }
                Ok(())
            })?;
        pixels
            .par_chunks_mut(w)
            .enumerate()
            .try_for_each(|(y, row)| -> Result<()> {
                check(cancel)?;
                for (x, p) in row.iter_mut().enumerate() {
                    *p = columns[x * h + y];
                }
                Ok(())
            })?;
    }
    check(cancel)
}
struct Diffusion {
    pre_h: u32,
    near: u32,
    far: u32,
    pre_b: u32,
    bloom: u32,
}
impl Diffusion {
    fn new(f: FilmEffects, full: [u32; 2]) -> Self {
        let unit = full[1] as f32 / 1080.;
        let near = (1. + 24. * f.halation_spread) * (0.6 + 0.6 * f.halation_smoothness) * unit;
        let radius = |v: f32| v.ceil() as u32;
        Self {
            pre_h: radius(2. * f.halation_smoothness * unit),
            near: radius(near),
            far: radius(near * (1. + 3. * f.halation_global_diffusion)),
            pre_b: radius(3. * (1. - f.bloom_details) * unit),
            bloom: radius((2. + 38. * f.bloom_spread * f.bloom_spread) * unit),
        }
    }
}
pub(crate) fn halo(f: FilmEffects, full: [u32; 2]) -> u32 {
    let d = Diffusion::new(f, full);
    let h = if f.halation_active() {
        d.pre_h.saturating_add(d.far.saturating_mul(3))
    } else {
        0
    };
    let b = if f.bloom_active() {
        d.pre_b.saturating_add(d.bloom.saturating_mul(3))
    } else {
        0
    };
    h.max(b).saturating_add(grain::halo(f, full))
}
pub(crate) struct Region {
    pub full: [u32; 2],
    pub origin: [u32; 2],
    pub output: [u32; 4],
    pub scale: [f32; 2],
}
fn crop(im: Rendered, [x, y, w, h]: [u32; 4]) -> Rendered {
    if [x, y, w, h] == [0, 0, im.width, im.height] {
        return im;
    }
    let pixels = (0..h)
        .flat_map(|row| {
            let a = ((row + y) * im.width + x) as usize;
            im.pixels[a..a + w as usize].iter().copied()
        })
        .collect();
    Rendered {
        width: w,
        height: h,
        pixels,
    }
}
/// Prepare broad diffusion with the caller's halo, then retain only the grain
/// margin. Publish atomically so failed/cancelled processing cannot alter input.
pub(crate) fn apply_region(
    im: &mut Rendered,
    f: FilmEffects,
    r: Region,
    view: View,
    cancel: &AtomicBool,
) -> Result<()> {
    check(cancel)?;
    let Region {
        full,
        origin,
        output: [x, y, w, h],
        scale,
    } = r;
    ensure!(
        im.width > 0 && im.height > 0 && im.pixels.len() == im.width as usize * im.height as usize,
        "Invalid film input"
    );
    ensure!(
        origin[0]
            .checked_add(im.width)
            .is_some_and(|v| v <= full[0])
            && origin[1]
                .checked_add(im.height)
                .is_some_and(|v| v <= full[1]),
        "Film input outside image"
    );
    ensure!(
        w > 0
            && h > 0
            && x >= origin[0]
            && y >= origin[1]
            && x.checked_add(w).is_some_and(|v| v <= origin[0] + im.width)
            && y.checked_add(h).is_some_and(|v| v <= origin[1] + im.height)
            && scale.iter().all(|v| v.is_finite() && *v > 0. && *v <= 1.),
        "Invalid film output"
    );
    if !f.active() && view == View::Image {
        let next = Rendered {
            width: im.width,
            height: im.height,
            pixels: std::mem::take(&mut im.pixels),
        };
        *im = crop(next, [x - origin[0], y - origin[1], w, h]);
        return Ok(());
    }
    let mut next = im.clone();
    compression::apply(&mut next.pixels, f, cancel)?;
    diffusion(&mut next, f, full, view, cancel)?;
    let pad = if view == View::Image {
        grain::halo(f, full)
    } else {
        0
    };
    let left = x.saturating_sub(pad).max(origin[0]);
    let top = y.saturating_sub(pad).max(origin[1]);
    let right = (x + w).saturating_add(pad).min(origin[0] + next.width);
    let bottom = (y + h).saturating_add(pad).min(origin[1] + next.height);
    next = crop(
        next,
        [
            left - origin[0],
            top - origin[1],
            right - left,
            bottom - top,
        ],
    );
    if view == View::Image {
        let size = [next.width, next.height];
        grain::apply(&mut next.pixels, size, full, [left, top], f, scale, cancel)?;
    }
    check(cancel)?;
    *im = crop(next, [x - left, y - top, w, h]);
    Ok(())
}
fn diffusion(
    im: &mut Rendered,
    f: FilmEffects,
    full: [u32; 2],
    view: View,
    cancel: &AtomicBool,
) -> Result<()> {
    let d = Diffusion::new(f, full);
    let size = [im.width, im.height];
    let h = f.halation_active() && view != View::Bloom;
    let b = f.bloom_active() && view != View::Halation;
    if !h && !b {
        if view != View::Image {
            im.pixels.fill([0.; 3]);
        }
        return Ok(());
    }
    let source = im.pixels.clone();
    let mut added = vec![[0.; 3]; source.len()];
    if h {
        let cutoff = (0.65 * f.halation_threshold + 0.25 * f.halation_source_limiter).min(0.95);
        let mut near: Vec<_> = source
            .par_iter()
            .map(|p| {
                let gate = ramp(luminance(*p), cutoff).powf(1. + 2. * f.halation_source_limiter);
                p.map(|v| v * gate)
            })
            .collect();
        blur(&mut near, size, d.pre_h, 1, cancel)?;
        let mut far = near.clone();
        blur(&mut near, size, d.near, 3, cancel)?;
        blur(&mut far, size, d.far, 3, cancel)?;
        let tint = [
            1.,
            0.12 + 0.72 * f.halation_hue,
            0.04 + 0.15 * f.halation_hue,
        ];
        let gain = f.halation_amount * (1. + 4. * f.halation_amplify * f.halation_amplify);
        added.par_iter_mut().enumerate().for_each(|(i, out)| {
            let background = f.halation_background_gain
                + (1. - f.halation_background_gain) * (1. - luminance(source[i]).clamp(0., 1.));
            for k in 0..3 {
                let field = near[i][k] + (far[i][k] - near[i][k]) * f.halation_global_diffusion;
                out[k] = gain * background * tint[k] * field;
            }
            out[2] -= 0.25 * f.halation_blue_compensation * out[0] * source[i][2];
        });
    }
    check(cancel)?;
    if b {
        let cutoff = 1. - 0.8 * f.bloom_threshold;
        let mut glow: Vec<_> = source
            .par_iter()
            .map(|p| {
                let l = luminance(*p);
                let gate = ramp(l, cutoff) + 0.03 * (1. - f.bloom_source_limiter) * l;
                p.map(|v| v * gate)
            })
            .collect();
        blur(&mut glow, size, d.pre_b, 1, cancel)?;
        blur(&mut glow, size, d.bloom, 3, cancel)?;
        let gain = f.bloom_amount * (0.5 + 3.5 * f.bloom_amplify * f.bloom_amplify);
        added.par_iter_mut().enumerate().for_each(|(i, out)| {
            let gray = luminance(glow[i]);
            let l = luminance(source[i]).clamp(0., 1.);
            let protection = 1. - f.bloom_protection * l * l;
            for (k, v) in out.iter_mut().enumerate() {
                let c = gray + (glow[i][k] - gray) * f.bloom_saturation;
                *v += gain * c * protection * (1. - source[i][k].clamp(0., 1.));
            }
        });
    }
    check(cancel)?;
    im.pixels
        .par_iter_mut()
        .zip(added)
        .for_each(|(pixel, light)| {
            *pixel = std::array::from_fn(|k| {
                if view == View::Image {
                    (pixel[k] + light[k]).clamp(0., 1.)
                } else {
                    light[k].clamp(0., 1.)
                }
            });
        });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_highlights_cutoff_keeps_clipped_white_finite() {
        let mut im = Rendered {
            width: 1,
            height: 1,
            pixels: vec![[1.; 3]],
        };
        let f = FilmEffects {
            bloom_amount: 1.,
            bloom_threshold: 0.,
            ..Default::default()
        };
        apply(&mut im, f, View::Image).unwrap();
        assert!(im.pixels[0].iter().all(|v| v.is_finite()));
    }
    #[test]
    fn diffusion_preserves_white_above_display_range_after_compression() {
        for halation in [true, false] {
            let mut im = Rendered {
                width: 1,
                height: 1,
                pixels: vec![[1.; 3]],
            };
            let f = FilmEffects {
                compression_enabled: true,
                compression_impact: 0.,
                compression_white_point: 1.2,
                halation_amount: if halation { 1. } else { 0. },
                halation_amplify: 1.,
                bloom_amount: if halation { 0. } else { 1. },
                bloom_amplify: 1.,
                bloom_protection: 0.,
                ..Default::default()
            };
            f.validate().unwrap();
            apply(&mut im, f, View::Image).unwrap();
            assert_eq!(im.pixels, vec![[1.; 3]], "halation={halation}");
        }
    }
    fn scene() -> Rendered {
        let mut im = Rendered {
            width: 128,
            height: 96,
            pixels: vec![[0.02; 3]; 128 * 96],
        };
        for y in 35..59 {
            for x in 51..77 {
                im.pixels[y * 128 + x] = [1.; 3];
            }
        }
        im
    }
    fn apply(im: &mut Rendered, f: FilmEffects, view: View) -> Result<()> {
        let full = [im.width, im.height];
        apply_region(
            im,
            f,
            Region {
                full,
                origin: [0, 0],
                output: [0, 0, full[0], full[1]],
                scale: [1.; 2],
            },
            view,
            &AtomicBool::new(false),
        )
    }
    #[test]
    fn boxes_preserve_constants_even_when_the_radius_exceeds_the_image() {
        for radius in [1, 5, 100] {
            let mut p = vec![[0.3, 0.7, 0.1]; 35];
            blur(&mut p, [7, 5], radius, 3, &AtomicBool::new(false)).unwrap();
            assert!(p.iter().all(|v| {
                v.iter()
                    .zip([0.3, 0.7, 0.1])
                    .all(|(a, b)| (*a - b).abs() < 1e-6)
            }));
        }
    }
    #[test]
    fn inactive_is_exact_and_cancellation_is_atomic() {
        let mut im = scene();
        let before = im.clone();
        apply(&mut im, FilmEffects::default(), View::Image).unwrap();
        assert_eq!(im.pixels, before.pixels);
        let f = FilmEffects {
            halation_amount: 0.7,
            ..Default::default()
        };
        assert!(
            apply_region(
                &mut im,
                f,
                Region {
                    full: [128, 96],
                    origin: [0, 0],
                    output: [0, 0, 128, 96],
                    scale: [1.; 2]
                },
                View::Image,
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert_eq!(im.pixels, before.pixels);
    }
    #[test]
    fn both_diffusion_layers_add_light_and_diagnostics_do_not_show_the_source() {
        let original = scene();
        for f in [
            FilmEffects {
                halation_amount: 0.7,
                ..Default::default()
            },
            FilmEffects {
                bloom_amount: 0.7,
                ..Default::default()
            },
        ] {
            let mut im = original.clone();
            apply(&mut im, f, View::Image).unwrap();
            assert_ne!(im.pixels, original.pixels);
            assert!(
                im.pixels
                    .iter()
                    .zip(&original.pixels)
                    .any(|(a, b)| a[0] > b[0] + 0.001)
            );
        }
        let mut im = original;
        apply(&mut im, FilmEffects::default(), View::Bloom).unwrap();
        assert!(im.pixels.iter().all(|p| *p == [0.; 3]));
    }
    #[test]
    fn public_film_stack_matches_padded_regions() {
        let f = FilmEffects {
            halation_amount: 0.6,
            bloom_amount: 0.4,
            grain_amount: 0.3,
            compression_enabled: true,
            ..Default::default()
        };
        let source = scene();
        let mut full = source.clone();
        apply(&mut full, f, View::Image).unwrap();
        let pad = halo(f, [128, 96]);
        for [x, y, w, h] in [[47u32, 29u32, 19, 23], [0, 0, 17, 15], [111, 81, 17, 15]] {
            let left = x.saturating_sub(pad);
            let top = y.saturating_sub(pad);
            let right = (x + w + pad).min(128);
            let bottom = (y + h + pad).min(96);
            let mut tile = crop(source.clone(), [left, top, right - left, bottom - top]);
            apply_region(
                &mut tile,
                f,
                Region {
                    full: [128, 96],
                    origin: [left, top],
                    output: [x, y, w, h],
                    scale: [1.; 2],
                },
                View::Image,
                &AtomicBool::new(false),
            )
            .unwrap();
            for yy in 0..h {
                for xx in 0..w {
                    for k in 0..3 {
                        assert!(
                            (tile.pixels[(yy * w + xx) as usize][k]
                                - full.pixels[((y + yy) * 128 + x + xx) as usize][k])
                                .abs()
                                < 4e-6
                        );
                    }
                }
            }
        }
    }
}
