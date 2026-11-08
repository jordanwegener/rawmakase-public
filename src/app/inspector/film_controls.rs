//! Additional film controls; the page choice is transient viewport UI state.
use super::*;
use crate::model::film::{CompressionFormat, GrainFormat};

const TAB: &str = "film-controls-tab";
pub(in crate::app) fn select(ctx: &egui::Context, tab: u8) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(TAB), tab.min(2)));
    ctx.request_repaint();
}
pub(in crate::app) fn selected(ctx: &egui::Context) -> u8 {
    ctx.data_mut(|data| data.get_temp::<u8>(egui::Id::new(TAB)).unwrap_or(0))
        .min(2)
}
pub(super) fn tabs(ui: &mut egui::Ui) -> (u8, bool) {
    let before = selected(ui.ctx());
    let mut tab = before;
    ui.horizontal_wrapped(|ui| {
        for (value, label) in [
            (0, "Halation / Bloom"),
            (1, "Film Grain"),
            (2, "Compression"),
        ] {
            ui.selectable_value(&mut tab, value, label);
        }
    });
    if before != tab {
        select(ui.ctx(), tab);
    }
    (tab, before != tab)
}

fn profile<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: Option<T>,
    all: &[T],
    label: impl Fn(T) -> &'static str,
    mut apply: impl FnMut(T),
) {
    control_row(ui, "Profile", |ui| {
        egui::ComboBox::from_id_salt("texture-profile")
            .selected_text(current.map(&label).unwrap_or("Custom"))
            .show_ui(ui, |ui| {
                for &value in all {
                    if ui
                        .selectable_label(current == Some(value), label(value))
                        .clicked()
                    {
                        apply(value);
                    }
                }
            });
    });
}
pub(super) fn additional(
    ui: &mut egui::Ui,
    r: &mut Recipe,
    metadata: Option<&crate::camera_data::Metadata>,
    tab: u8,
) {
    let defaults = crate::model::film::FilmEffects::default();
    ui.push_id(("film-controls", tab), |ui| match tab {
        1 => {
            film_switch(ui, "Film Grain", &mut r.effects.film.grain_enabled);
            ui.add_enabled_ui(r.effects.film.grain_enabled, |ui| {
                profile(
                    ui,
                    r.effects.film.grain_format(),
                    &GrainFormat::ALL,
                    GrainFormat::label,
                    |p| r.effects.film.apply_grain_preset(p),
                );
                for (id, default) in [
                    (ParameterId::FilmGrainAmount, defaults.grain_amount),
                    (ParameterId::FilmGrainSize, defaults.grain_size),
                    (ParameterId::FilmGrainShadows, defaults.grain_shadows),
                    (ParameterId::FilmGrainMidtones, defaults.grain_midtones),
                    (ParameterId::FilmGrainHighlights, defaults.grain_highlights),
                    (ParameterId::FilmGrainColor, defaults.grain_color),
                    (ParameterId::FilmGrainResolution, defaults.grain_resolution),
                ] {
                    setting_control(ui, r, id, default, metadata);
                }
                control_row(ui, "Pattern", |ui| {
                    if ui.small_button("New Pattern").clicked() {
                        r.effects.film.grain_seed = r.effects.film.grain_seed.wrapping_add(1);
                        name_history_step(
                            ui,
                            "Film Grain Pattern".into(),
                            r.effects.film.grain_seed.to_string(),
                        );
                    }
                });
            });
        }
        2 => {
            film_switch(
                ui,
                "Film Compression",
                &mut r.effects.film.compression_enabled,
            );
            ui.add_enabled_ui(r.effects.film.compression_enabled, |ui| {
                profile(
                    ui,
                    r.effects.film.compression_format(),
                    &CompressionFormat::ALL,
                    CompressionFormat::label,
                    |p| r.effects.film.apply_compression_preset(p),
                );
                for (id, default) in [
                    (
                        ParameterId::FilmCompressionImpact,
                        defaults.compression_impact,
                    ),
                    (
                        ParameterId::FilmCompressionRange,
                        defaults.compression_range,
                    ),
                    (
                        ParameterId::FilmCompressionWhitePoint,
                        defaults.compression_white_point,
                    ),
                    (
                        ParameterId::FilmCompressionColorDensity,
                        defaults.compression_color_density,
                    ),
                ] {
                    setting_control(ui, r, id, default, metadata);
                }
            });
        }
        _ => {}
    });
}
