# Film Effects

The public edition provides Halation, Bloom, Film Grain and Film Compression.
Three control pages group Halation/Bloom, Film Grain and Compression. Each effect
has an independent saved switch; the section switch bypasses all four without
changing their controls. The preview diagnostics are transient UI state.

A new recipe bypasses every effect: Halation, Bloom and Film Grain have zero
amount, and Film Compression is disabled with zero impact. The shared section
switch and the Halation, Bloom and Grain switches begin enabled. Shape controls
are neutral starting values; the grain pattern starts at seed 42.

The format profiles and bundled layerable presets are original aesthetic starting
points authored for this edition. They do not reproduce a commercial product,
measured film stock, or vendor behavior. The format and ISO labels describe a
useful editing progression, not a calibrated physical simulation.

Halation and Bloom use a format coordinate from zero for 65 mm to one for 8 mm,
with evenly spaced 35 mm and 16 mm steps. Smaller formats increase spread and
strength. The No Remjet Halation choices lower the source limiter and broaden the
halo. Grain uses the same format coordinate and a separate ISO coordinate
(50 → 0, 250 → 0.5, 500 → 1): smaller formats and higher ISO increase grain size
and amount, while reducing resolution. Midtones receive the most grain by
default; color texture is restrained. Compression's Soft, Dense and Faded
profiles follow an original strength axis. Faded also lowers white point and
uses less color density. These formulas live with the model and are independent
of the rendering implementation.

Choosing a profile changes only that effect, preserves the shared section switch
and retains the grain seed. New Pattern changes the seed as one undoable edit.
Undo/Redo, Copy Settings, Sync and preset Amount retain the same recipe semantics
as other controls. Copy/Sync exposes four independent Film Effects groups; each
selected group carries the shared section switch as well as its own settings.

The bundled Film Effects XMP presets layer over an existing look. Individual
Halation and Bloom presets write only their own child packet; the three combined
presets write only Halation/Bloom. They preserve white balance, profile, tone,
color, legacy Grain and the other native film effects. Preset names and UUIDs are
stable while their values are the newly authored public starting points.

User-imported profiles and XMP presets remain supported within the public
feature set. Full native sidecars use `rawmakase:FilmEffects`; effect presets use
`FilmHalation`, `FilmBloom`, `FilmGrain` or `FilmCompression`. The older
`FilmDiffusion` packet and saved selection still mean Halation/Bloom only.
Unknown native fields and unsupported native effect packets are rejected by
strict loading/application. Best-effort XMP import reports unsupported settings
as warnings. Experimental private Film Damage recipes and presets are outside
public compatibility, including full packets containing its disabled fields;
they are not silently converted into four-effect edits.

No Film Damage controls, parameters, groups, presets, renderer or bundled textures
are included. No proprietary shader source, recovered reference fixtures, image
textures or purchased presets belong in this tree.

Grain uses each output axis's actual size relative to native geometry, including
when a very thin image keeps one pixel on its short axis. Preview filtering
integrates broad clumps through the same Lanczos3 footprint as reduced exports;
unresolved finer bands use bounded statistical filtering.
