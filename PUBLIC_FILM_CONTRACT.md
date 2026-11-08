# Authoring contract

Implement newly authored CPU Film Grain and Film Compression using only the functional requirements in your task and this interface. Do not inspect another project, proprietary research, git history, or discarded operators. No exact-matching goal or supplied output fixtures.

Available types:
- `crate::model::film::FilmEffects`, a Copy struct with normalized f32 controls and saved u64 seed. Grain fields: grain_amount, grain_size, grain_shadows, grain_midtones, grain_highlights, grain_color, grain_resolution, grain_seed. Methods grain_active() and compression_active(). Compression fields compression_impact, compression_range, compression_white_point (0.5..1.2), compression_color_density.
- `super::check(&AtomicBool)->anyhow::Result<()>` checks cancellation.
- `super::blur(&mut [[f32;3]], [u32;2], radius:u32, passes:u32, &AtomicBool)->Result<()>` separable clamped box blur; finite support radius*passes. Three passes approximate Gaussian. Original common implementation provided by parent.

Grain interface:
`pub(super) fn halo(f:FilmEffects,full:[u32;2])->u32`
`pub(super) fn apply(pixels:&mut[[f32;3]],size:[u32;2],full:[u32;2],origin:[u32;2],f:FilmEffects,output_scale:f32,cancel:&AtomicBool)->Result<()>`
Full dimensions/origin use the entire uncropped output grid; output_scale is output pixels per native photo pixel. Exports and 100% use 1. Preview must reduce grain RMS and apparent clump width consistently with reducing a native image. Whole/crop agreement within 4e-6 with supplied halo. Original seeded coordinate noise only, no assets. Color=0 must add neutral grain; preserve input chroma. Amount=0 exact bypass. All seven controls visibly affect output. Finite output, check cancellation in spatial loops. Runtime near O(output pixels), no full-frame noise canvas for crops.

Compression interface:
`pub(super) fn apply(pixels:&mut[[f32;3]],f:FilmEffects,cancel:&AtomicBool)->Result<()>`
A monotonic, smooth highlight shoulder with configurable onset (Range), blend (Impact), output White Point and highlight Color Density. Maintain hue sensibly. No table copied from a product, no source lookup. Full finite-domain behavior defined explicitly and covered by synthetic tests. Disabled is exact bypass. New source code and original analytic behavior tests only; document formulas.
