# Source and asset provenance

This unofficial community edition retains the upstream MIT copyright and permission notice in `LICENSE`. Its upstream history is retained. The first community commit contains the additions in their public form; discarded experimental implementations are not part of this repository's history.

## Film implementation

| Component | Source |
| --- | --- |
| Editor, model, XMP, profiles/import, GPU colour and core develop pipeline | Upstream RAWmakase, MIT; community integration changes under the same license. |
| Halation / Bloom | Newly authored highlight masks, coloured light addition, screen-style bloom and three finite box passes. Prefix sums give work linear in pixel count and independent of diffusion radius. |
| Film Grain | Newly authored seeded coordinate lattice noise, tonal Bernstein weights, opponent colour axes and statistical area-average preview attenuation. No image texture or measured response table. |
| Film Compression | Newly authored analytic smooth rational shoulder and hue-preserving RGB scaling. No sampled response table. |
| Format profiles and layer presets | Original formula-based starting points authored for these operators. They do not promise calibrated physical stocks or matching another renderer. |
| Film Damage | Excluded: no implementation, control, texture bundle, preset or asset loader. |
| Film tests | Original analytic and synthetic invariants; no external-renderer film fixtures. |

Grain and Compression were authored in fresh implementation contexts from a functional API contract, without access to prior implementations, proprietary source, research or reference outputs. Halation/Bloom were newly written from the equations documented below. This records engineering provenance; it is not a formal clean-room certification or legal opinion.

The private development catalog, photographs, purchased preset/profile collections, recovered source and extracted resources are not distributed. Users import their own properly licensed profiles. Imported profiles are never automatically included in an application release.

## Mathematical definitions

A diffusion pass is a normalized separable box average with clamped image edges. Three passes approximate a Gaussian. The support is exactly three times the pass radius; preceding source smoothing and Grain support are added before cropping. No image-size-dependent native canvas or proprietary kernel is used.

Halation extracts a smooth highlight mask, averages it at local and wider radii, colours the result and adds it to the developed photo. Bloom averages a highlight-selected source and adds a luminance-protected, saturation-adjustable screen-style contribution. All coefficients and profile progressions in this edition were chosen for this implementation.

Grain uses integer-hashed lattice values with quintic interpolation. Its preview footprint projects resolvable bands through separable Lanczos3 weights onto the quintic lattice. Subpixel bands use statistical attenuation based on interpolation covariance and each axis's actual reduction ratio. Compression joins the identity to `k + t / sqrt(1 + (t/(1-k))^2)` above knee `k`, then blends by Impact and scales by White Point. These formulas and their behavior tests are maintained with the code.

## Dependencies and release packaging

Dependencies retain their own licenses; `Cargo.lock`, `deny.toml`, `licenses/` and `THIRD_PARTY_NOTICES.txt` identify the distribution obligations. The MIT license does not relicense dependencies, fonts or separately imported user content. Release packaging must include the notices. The inherited website source is reference material; website, stats and external package-repository publishing are disabled.

Update and bug-report URLs point to this fork. Its update verification public key is separate from upstream's; signing credentials are never in Git. No signed binary release is implied by source availability. Platform packaging and signing require their own release validation.
