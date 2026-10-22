use crate::{app::Parfait, brushes::Preset, workspace::Workspace};
use serde::{Deserialize, Serialize};
pub(crate) const VERSION: u32 = 25;

#[derive(Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub options: crate::settings::Options,
    #[serde(default)]
    pub keymap: crate::shortcuts::Keymap,
    #[serde(default)]
    pub display: crate::display::DisplaySettings,
    #[serde(default)]
    pub calibration: parfait_core::PressureCalibration,
    pub version: u32,
    #[serde(default)]
    pub tools: crate::tools::ToolSettings,
    pub workspace: Workspace,
    pub presets: Vec<Preset>,
    pub brush: parfait_core::Brush,
    pub tip_name: String,
    #[serde(default)]
    pub pressure_tab: bool,
    #[serde(default)]
    pub brush_library: crate::brushes::BrushLibrary,
    #[serde(default)]
    pub property_layout: crate::brush_details::PropertyLayout,
}

impl Preferences {
    fn current(app: &Parfait) -> Self {
        Self {
            options: app.options.clone(),
            calibration: app.calibration.clone(),
            version: VERSION,
            tools: {
                let mut t = app.tools.clone();
                t.drag = None;
                t
            },
            keymap: app.keymap.preferences(),
            display: app.display,
            workspace: app.workspace.clone(),
            presets: app.presets.clone(),
            brush: app.brush.clone(),
            tip_name: app.tip_name.clone(),
            pressure_tab: app.pressure_tab,
            brush_library: app.brush_library.preferences(),
            property_layout: app.brush_details.properties,
        }
    }

    pub(crate) fn matches(&self, app: &Parfait) -> bool {
        self.version == VERSION
            && self.tools == app.tools
            && self.options == app.options
            && self.keymap == app.keymap
            && self.display == app.display
            && self.calibration == app.calibration
            && self.workspace.same_preferences(&app.workspace)
            && self.presets == app.presets
            && self.brush == app.brush
            && self.tip_name == app.tip_name
            && self.pressure_tab == app.pressure_tab
            && self.brush_library.group == app.brush_library.group
            && self.brush_library.favorites_only
                == app.brush_library.favorites_only
            && self.property_layout == app.brush_details.properties
    }

    pub fn validate(&self) -> bool {
        (1..=VERSION).contains(&self.version)
            && self.tools.validate()
            && self.options.validate()
            && self.keymap.validate()
            && self.display.validate()
            && self.calibration.validate()
            && self.workspace.validate()
            && self.presets.len() <= 64
            && self
                .presets
                .iter()
                .all(|p| p.brush.validate() && p.name.len() <= 128)
            && self.brush.validate()
            && self.tip_name.len() <= 128
    }
}
impl Parfait {
    pub fn restore_preferences(&mut self) {
        let result = (|| -> anyhow::Result<()> {
            if let Some(json) = parfait_storage::load_preferences(
                &self.recovery_path.with_file_name("preferences.sqlite"),
            )? {
                anyhow::ensure!(
                    json.len() <= 32 * 1024 * 1024,
                    "Preferences too large"
                );
                let mut pref: Preferences = serde_json::from_str(&json)?;
                if (1..=9).contains(&pref.version) {
                    pref.workspace.upgrade_history();
                }
                if pref.version < 24 {
                    pref.workspace.upgrade_tools();
                }
                if pref.version < 25 {
                    pref.workspace.upgrade_columns();
                }
                anyhow::ensure!(
                    pref.validate(),
                    "Invalid or unsupported preferences"
                );
                if pref.version < 23 {
                    crate::brushes::upgrade_default_spacing(
                        &mut pref.presets,
                        &mut pref.brush,
                        &pref.tip_name,
                    );
                    pref.tools.category =
                        crate::tools::ToolCategory::from_brush(
                            pref.brush_library.group.unwrap_or_else(|| {
                                crate::brushes::BrushGroup::of(&pref.brush)
                            }),
                        );
                }
                self.tools = pref.tools.clone();
                self.navigation_tool = self
                    .tools
                    .subtools
                    .iter()
                    .find(|t| Some(t.id) == self.tools.active_id)
                    .map_or(0, |t| t.kind as u8);
                self.options = pref.options.clone();
                self.calibration = pref.calibration.clone();
                self.display = pref.display;
                self.keymap = pref.keymap.clone();
                self.keymap.refresh();
                self.workspace = pref.workspace.clone();
                self.presets = pref.presets.clone();
                self.brush = pref.brush.clone();
                self.tip_name = pref.tip_name.clone();
                self.pressure_tab = pref.pressure_tab;
                self.brush_library = pref.brush_library.preferences();
                self.brush_details.properties = pref.property_layout;
                self.preferences_saved = Some(pref);
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.preferences_read_failed = true;
            self.error = Some(format!(
                "Preferences could not be loaded and will not be overwritten: {e}"
            ));
        }
    }
    pub fn persist_preferences(&mut self, force: bool) {
        if (!force
            && (self.tools.drag.is_some()
                || (!self.documents.is_empty()
                    && self
                        .documents
                        .active_session
                        .editing
                        .layer_edit
                        .is_some())))
            || (!force
                && !self.documents.is_empty()
                && self.documents.active_session.editing.stroke.is_some())
            || self.preferences_read_failed
            || (!force && self.preferences_tick.elapsed().as_secs_f32() < 1.)
        {
            return;
        }
        self.preferences_tick = std::time::Instant::now();
        if self
            .preferences_saved
            .as_ref()
            .is_some_and(|saved| saved.matches(self))
        {
            return;
        }
        let prefs = Preferences::current(self);
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(prefs.validate(), "Invalid preferences");
            let json = serde_json::to_string(&prefs)?;
            parfait_storage::save_preferences(
                &self.recovery_path.with_file_name("preferences.sqlite"),
                &json,
            )?;
            self.preferences_saved = Some(prefs);
            Ok(())
        })();
        if let Err(e) = result {
            self.error =
                Some(format!("Workspace/brush settings save failed: {e}"));
        }
    }

    #[cfg(test)]
    pub(crate) fn saved_preferences_json(&self) -> String {
        parfait_storage::load_preferences(
            &self.recovery_path.with_file_name("preferences.sqlite"),
        )
        .unwrap()
        .unwrap()
    }
}

#[cfg(test)]
mod history_migration_tests {
    use super::*;
    use crate::workspace::{Location, Palette};

    #[test]
    fn version_23_adds_detachable_tools_and_restores_separate_window_preferences()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.workspace.left_width = 320.;
        app.persist_preferences(true);
        let mut legacy: serde_json::Value =
            serde_json::from_str(&app.saved_preferences_json()).unwrap();
        legacy["version"] = 23.into();
        legacy["workspace"]["panels"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p["palette"] != "Tools");
        let expected = legacy["workspace"]["panels"].clone();
        drop(app);
        parfait_storage::save_preferences(
            &dir.path().join("preferences.sqlite"),
            &legacy.to_string(),
        )
        .unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert!(!app.preferences_read_failed);
        assert_eq!(app.workspace.left_width, 320.);
        assert_eq!(
            &serde_json::to_value(&app.workspace).unwrap()["panels"]
                .as_array()
                .unwrap()[..7],
            expected.as_array().unwrap().as_slice()
        );
        let tools = app
            .workspace
            .panels
            .iter_mut()
            .find(|p| p.palette == Palette::Tools)
            .unwrap();
        assert_eq!(tools.location, Location::Rail);
        tools.location = Location::Window;
        tools.window_rect = Some([140., 120., 64., 329.]);
        tools.inactive_opacity = 0.6;
        app.persist_preferences(true);
        drop(app);
        let app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert!(!app.preferences_read_failed);
        let tools = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Tools)
            .unwrap();
        assert_eq!(tools.location, Location::Window);
        assert_eq!(tools.window_rect, Some([140., 120., 64., 329.]));
        assert_eq!(tools.inactive_opacity, 0.6);
    }

    #[test]
    fn six_palette_versions_upgrade_without_moving_existing_layouts_or_resetting_keys()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.workspace.panels[0].location = Location::Floating;
        app.workspace.panels[0].rect = [130., 90., 320., 410.];
        app.persist_preferences(true);
        let mut legacy: serde_json::Value =
            serde_json::from_str(&app.saved_preferences_json()).unwrap();
        legacy["workspace"]["panels"].as_array_mut().unwrap().retain(
            |placement| {
                placement["palette"] != "History"
                    && placement["palette"] != "Tools"
            },
        );
        let expected = legacy["workspace"]["panels"].clone();
        let keys = app.keymap.clone();
        drop(app);
        let path = dir.path().join("preferences.sqlite");
        for version in 1..=9 {
            legacy["version"] = version.into();
            parfait_storage::save_preferences(&path, &legacy.to_string())
                .unwrap();
            let mut restored =
                Parfait::open_legacy(dir.path().to_owned()).unwrap();
            assert!(!restored.preferences_read_failed);
            assert!(restored.keymap == keys);
            assert_eq!(restored.workspace.panels.len(), 8);
            let history = restored
                .workspace
                .panels
                .iter()
                .find(|placement| placement.palette == Palette::History)
                .unwrap();
            assert!(!history.visible);
            let actual = serde_json::to_value(&restored.workspace).unwrap();
            assert_eq!(
                &actual["panels"].as_array().unwrap()[..6],
                expected.as_array().unwrap().as_slice()
            );
            restored.persist_preferences(true);
            assert_eq!(
                restored.preferences_saved.as_ref().unwrap().version,
                VERSION
            );
        }
        legacy["version"] = 10.into();
        parfait_storage::save_preferences(&path, &legacy.to_string()).unwrap();
        let mut restored = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert!(restored.preferences_read_failed);
        restored.persist_preferences(true);
        assert_eq!(restored.saved_preferences_json(), legacy.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_preferences_cover_persisted_fields_and_ignore_session_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        app.persist_preferences(true);
        let changes: [fn(&mut Parfait); 24] = [
            |app| app.brush.opacity_mode = parfait_core::OpacityMode::PerStroke,
            |app| {
                app.keymap.remove(
                    crate::shortcuts::Command::ToolInk,
                    crate::shortcuts::Chord::new(
                        eframe::egui::Key::P,
                        false,
                        false,
                        false,
                    ),
                )
            },
            |app| app.display.rulers = !app.display.rulers,
            |app| app.display.pixel_grid = !app.display.pixel_grid,
            |app| app.display.brush_cursor = !app.display.brush_cursor,
            |app| app.display.background = 80,
            |app| app.display.rotation_step = 15.,
            |app| app.brush.operation = parfait_core::BrushOperation::Blur,
            |app| app.brush.color_stretch = 0.1,
            |app| app.brush.blur_automatic = false,
            |app| app.brush.blur_radius = 12.,
            |app| {
                app.brush_details.properties =
                    crate::brush_details::PropertyLayout::blending(false)
            },
            |app| app.calibration.points.insert(1, [0.5, 0.8]),
            |app| app.workspace.left_width += 10.,
            |app| {
                app.workspace.panels[0].collapsed =
                    !app.workspace.panels[0].collapsed
            },
            |app| app.presets[0].favorite = true,
            |app| app.presets[0].name.push_str(" changed"),
            |app| app.presets[0].brush.size += 1.,
            |app| app.brush.size += 1.,
            |app| app.tip_name.push_str(" changed"),
            |app| app.pressure_tab = !app.pressure_tab,
            |app| app.brush_library.group = None,
            |app| app.brush_library.favorites_only = true,
            |app| {
                app.brush_details.properties = serde_json::from_value(
                    serde_json::json!({"visible":vec![false;9]}),
                )
                .unwrap()
            },
        ];
        for change in changes {
            app.restore_preferences();
            let saved = app.saved_preferences_json();
            app.brush_library.query = "session-only search".into();
            app.workspace.dragging = Some(crate::workspace::Palette::Color);
            assert!(app.preferences_saved.as_ref().unwrap().matches(&app));
            change(&mut app);
            assert!(!app.preferences_saved.as_ref().unwrap().matches(&app));
            assert_ne!(
                serde_json::to_string(&Preferences::current(&app)).unwrap(),
                saved
            );
            app.persist_preferences(true);
            assert!(app.error.is_none());
            assert!(app.preferences_saved.as_ref().unwrap().matches(&app));
            assert_eq!(
                app.saved_preferences_json(),
                serde_json::to_string(&Preferences::current(&app)).unwrap()
            );
        }
    }

    #[test]
    fn unchanged_preferences_skip_io_and_failed_or_invalid_edits_remain_retryable()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        app.persist_preferences(true);
        let original = app.saved_preferences_json();
        app.brush.size = f32::NAN;
        app.persist_preferences(true);
        assert!(app.error.take().is_some());
        assert_eq!(app.saved_preferences_json(), original);
        app.brush.size = 12.;
        let path = dir.path().join("preferences.sqlite");
        let backup = dir.path().join("preferences.backup");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();
        app.persist_preferences(true);
        assert!(app.error.is_none());
        app.brush.size = 41.;
        app.persist_preferences(true);
        assert!(app.error.take().is_some());
        assert_eq!(app.preferences_saved.as_ref().unwrap().brush.size, 12.);
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        app.persist_preferences(true);
        assert!(app.error.is_none());
        drop(app);
        let restored = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        assert_eq!(restored.brush.size, 41.);
    }

    #[test]
    fn legacy_cache_upgrades_and_future_preferences_remain_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        app.persist_preferences(true);
        let mut json: serde_json::Value =
            serde_json::from_str(&app.saved_preferences_json()).unwrap();
        let path = dir.path().join("preferences.sqlite");
        json["version"] = serde_json::json!(1);
        parfait_storage::save_preferences(&path, &json.to_string()).unwrap();
        app.restore_preferences();
        app.persist_preferences(true);
        assert_eq!(app.preferences_saved.as_ref().unwrap().version, 22);
        json["version"] = serde_json::json!(999);
        let future = json.to_string();
        parfait_storage::save_preferences(&path, &future).unwrap();
        app.restore_preferences();
        assert!(app.preferences_read_failed);
        app.brush.size = 99.;
        app.persist_preferences(true);
        assert_eq!(app.saved_preferences_json(), future);
    }

    #[test]
    #[ignore = "release-only maintenance timing workload"]
    fn unchanged_material_library_poll_benchmark() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        app.documents.active_session.document =
            parfait_core::Document::new(16, 16);
        let mask = std::sync::Arc::new(vec![128; 256 * 256]);
        app.presets = (0..64)
            .map(|index| Preset {
                name: format!("Benchmark material {index}"),
                brush: parfait_core::Brush {
                    tip: parfait_core::BrushTip::Image {
                        width: 256,
                        height: 256,
                        mask: mask.clone(),
                    },
                    ..Default::default()
                },
                favorite: false,
                properties: Default::default(),
            })
            .collect();
        app.persist_preferences(true);
        assert!(app.error.is_none());
        let bytes = parfait_storage::load_preferences(
            &dir.path().join("preferences.sqlite"),
        )
        .unwrap()
        .unwrap()
        .len();
        let start = std::time::Instant::now();
        for _ in 0..20 {
            app.persist_preferences(true);
            std::hint::black_box(&app);
        }
        let elapsed = start.elapsed();
        assert!(app.error.is_none());
        eprintln!(
            "unchanged preferences: json_bytes={bytes}, polls=20, total_ms={:.3}, ms_per_poll={:.6}",
            elapsed.as_secs_f64() * 1000.,
            elapsed.as_secs_f64() * 50.
        );
    }

    #[test]
    fn display_settings_roundtrip_legacy_defaults_and_invalid_values_are_protected()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.display = crate::display::DisplaySettings {
            rulers: false,
            pixel_grid: true,
            brush_cursor: false,
            background: 72,
            rotation_step: 17.,
        };
        app.documents.active_session.view.flip_horizontal = true;
        app.persist_preferences(true);
        let expected = app.display;
        let json = app.saved_preferences_json();
        drop(app);
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert_eq!(app.display, expected);
        assert!(!app.documents.active_session.view.flip_horizontal);
        let path = dir.path().join("preferences.sqlite");
        for version in 1..=6 {
            let mut legacy: serde_json::Value =
                serde_json::from_str(&json).unwrap();
            legacy["version"] = version.into();
            legacy.as_object_mut().unwrap().remove("display");
            parfait_storage::save_preferences(&path, &legacy.to_string())
                .unwrap();
            app.restore_preferences();
            assert!(!app.preferences_read_failed);
            assert_eq!(app.display, Default::default());
        }
        for display in [
            serde_json::json!({"rotation_step":0}),
            serde_json::json!({"rotation_step":91}),
            serde_json::json!({"background":19}),
            serde_json::json!({"background":97}),
        ] {
            let mut invalid: serde_json::Value =
                serde_json::from_str(&json).unwrap();
            invalid["display"] = display;
            let invalid = invalid.to_string();
            parfait_storage::save_preferences(&path, &invalid).unwrap();
            app.restore_preferences();
            assert!(app.preferences_read_failed);
            app.persist_preferences(true);
            assert_eq!(app.saved_preferences_json(), invalid);
        }
        assert!(
            !crate::display::DisplaySettings {
                rotation_step: f32::NAN,
                ..Default::default()
            }
            .validate()
        );
    }

    #[test]
    fn keymap_restart_legacy_defaults_and_corrupt_data_protection() {
        use crate::shortcuts::{Chord, Command, Keymap};
        use eframe::egui;
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.keymap
            .assign(
                Command::ToolInk,
                Some(Chord::new(egui::Key::P, false, false, false)),
                Chord::new(egui::Key::F8, false, false, false),
                false,
            )
            .unwrap();
        app.persist_preferences(true);
        let json = app.saved_preferences_json();
        drop(app);
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert_eq!(
            app.keymap
                .resolve(Chord::new(egui::Key::F8, false, false, false), None),
            Some(Command::ToolInk)
        );
        let path = dir.path().join("preferences.sqlite");
        for version in 1..=7 {
            let mut legacy: serde_json::Value =
                serde_json::from_str(&json).unwrap();
            legacy["version"] = version.into();
            legacy.as_object_mut().unwrap().remove("keymap");
            parfait_storage::save_preferences(&path, &legacy.to_string())
                .unwrap();
            app.restore_preferences();
            assert!(!app.preferences_read_failed);
            assert!(app.keymap == Keymap::default());
        }
        let mut invalid: serde_json::Value =
            serde_json::from_str(&json).unwrap();
        invalid["keymap"]["bindings"][0]["chord"]["key"] = "Space".into();
        let invalid = invalid.to_string();
        parfait_storage::save_preferences(&path, &invalid).unwrap();
        app.restore_preferences();
        assert!(app.preferences_read_failed);
        app.persist_preferences(true);
        assert_eq!(app.saved_preferences_json(), invalid);
    }
}

#[cfg(test)]
mod empty_workspace_tests {
    use super::*;
    #[test]
    fn periodic_preferences_persist_after_the_last_document_closes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let id = app.documents.active_id().unwrap();
        assert!(app.close_document_tab(id));
        app.preferences_tick =
            std::time::Instant::now() - std::time::Duration::from_secs(2);
        app.persist_preferences(false);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(app.preferences_saved.as_ref().unwrap().matches(&app));
        app.request_close(&eframe::egui::Context::default());
        assert!(app.recovery_runtime.as_ref().unwrap().shutdown_ready);
    }
}
