#[cfg(test)]
use eframe::egui::Rect;
use eframe::egui::{self, Sense, Stroke};
use parfait_core::{Brush, GapMode};

const MODES: [(GapMode, &str, usize, f32); 4] = [
    (GapMode::Fixed, "Fixed", 1, 0.),
    (GapMode::Wide, "Wide", 2, 11.),
    (GapMode::Normal, "Normal", 3, 7.),
    (GapMode::Narrow, "Narrow", 4, 4.),
];

pub fn gap_selector(ui: &mut egui::Ui, brush: &mut Brush) {
    ui.horizontal(|ui| {
        ui.label("Gap");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.;
            for (mode, label, count, distance) in MODES.into_iter().rev() {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(24., 24.), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        format!("Gap: {label}"),
                    )
                });
                let active = brush.gap_mode == mode;
                let visuals = ui.style().interact_selectable(&response, active);
                ui.painter().rect(
                    rect,
                    0.,
                    visuals.bg_fill,
                    Stroke::new(1., visuals.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                let start = rect.center() - egui::vec2(distance * (count - 1) as f32 * 0.5, 0.);
                for dot in 0..count {
                    ui.painter().circle_filled(
                        start + egui::vec2(dot as f32 * distance, 0.),
                        3.,
                        visuals.text_color(),
                    );
                }
                if response.clicked() {
                    brush.gap_mode = mode;
                }
                #[cfg(test)]
                ui.data_mut(|data| data.insert_temp(egui::Id::new(("gap_mode", label)), rect));
                response.on_hover_text(format!(
                    "{}\nCurrent interval: {:.2} document px",
                    match mode {
                        GapMode::Fixed => "Fixed: use your spacing value below",
                        GapMode::Wide => "Wide: 50% of the maximum brush diameter",
                        GapMode::Normal => "Normal: 25% of the maximum brush diameter",
                        GapMode::Narrow => {
                            "Narrow: 10% of the maximum brush diameter; more dabs cost more work"
                        }
                    },
                    brush.spacing_pixels()
                ));
            }
        });
    });
}

pub fn spray_controls(ui: &mut egui::Ui, brush: &mut Brush) {
    ui.separator();
    let response = ui.checkbox(&mut brush.continuous_spray, "Continuous spraying").on_hover_text("Keep depositing paint while the pen stays down, even without moving. Flow controls each dab; Rate controls dabs per second.");
    #[cfg(test)]
    ui.data_mut(|data| data.insert_temp(egui::Id::new("continuous_spray"), response.rect));
    let _ = response;
    ui.push_id("spray_rate", |ui| {
        ui.add_enabled_ui(brush.continuous_spray, |ui| {
            crate::brush_widgets::parameter(
                ui,
                &mut brush.spray_rate,
                crate::brush_widgets::Parameter {
                    label: "Spray rate",
                    range: 1. ..=120.,
                    decimals: 0,
                    unit: " dabs/s",
                    logarithmic: false,
                    dynamics: None,
                    layout: crate::brush_widgets::ParameterLayout::Detail,
                },
            );
        });
    });
    ui.collapsing("Other stroke settings", |ui| {
        ui.label("Spacing uses maximum diameter. Narrow gaps and higher spray rates do more work. The preview does not simulate a stationary hold.");
        ui.label("Ribbon, multiple-tip repeat order and pressure/velocity-driven spacing are not implemented yet.");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_buttons_preserve_fixed_value_and_spray_toggle_changes_only_its_setting() {
        let ctx = egui::Context::default();
        let mut brush = Brush {
            size: 10.,
            spacing: 2.5,
            ..Default::default()
        };
        let frame = |brush: &mut Brush, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    gap_selector(ui, brush);
                    spray_controls(ui, brush);
                },
            );
            output.textures_delta.clear();
        };
        frame(&mut brush, vec![]);
        frame(&mut brush, vec![]);
        let click = |brush: &mut Brush, id| {
            let center = ctx.data(|data| data.get_temp::<Rect>(id)).unwrap().center();
            let event = |pressed| egui::Event::PointerButton {
                pos: center,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            frame(brush, vec![egui::Event::PointerMoved(center), event(true)]);
            frame(brush, vec![event(false)]);
        };
        for (label, mode, pixels) in [
            ("Wide", GapMode::Wide, 5.),
            ("Normal", GapMode::Normal, 2.5),
            ("Narrow", GapMode::Narrow, 1.),
            ("Fixed", GapMode::Fixed, 25.),
        ] {
            click(&mut brush, egui::Id::new(("gap_mode", label)));
            assert_eq!(brush.gap_mode, mode);
            assert_eq!(brush.spacing_pixels(), pixels);
            assert_eq!(brush.spacing, 2.5);
        }
        let before = brush.clone();
        click(&mut brush, egui::Id::new("continuous_spray"));
        assert!(brush.continuous_spray);
        brush.continuous_spray = false;
        assert!(brush == before);
    }
}
