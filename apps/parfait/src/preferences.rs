use crate::{app::Parfait, brushes::Preset, workspace::Workspace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub calibration: parfait_core::PressureCalibration,
    pub version: u32,
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
            calibration: app.calibration.clone(),
            version: 4,
            workspace: app.workspace.clone(),
            presets: app.presets.clone(),
            brush: app.brush.clone(),
            tip_name: app.tip_name.clone(),
            pressure_tab: app.pressure_tab,
            brush_library: app.brush_library.preferences(),
            property_layout: app.brush_details.properties,
        }
    }

    fn matches(&self, app: &Parfait) -> bool {
        self.version == 4
            && self.calibration == app.calibration
            && self.workspace.same_preferences(&app.workspace)
            && self.presets == app.presets
            && self.brush == app.brush
            && self.tip_name == app.tip_name
            && self.pressure_tab == app.pressure_tab
            && self.brush_library.group == app.brush_library.group
            && self.brush_library.favorites_only == app.brush_library.favorites_only
            && self.property_layout == app.brush_details.properties
    }

    pub fn validate(&self) -> bool {
        (1..=4).contains(&self.version)
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
                anyhow::ensure!(json.len() <= 32 * 1024 * 1024, "Preferences too large");
                let pref: Preferences = serde_json::from_str(&json)?;
                anyhow::ensure!(pref.validate(), "Invalid or unsupported preferences");
                self.calibration = pref.calibration.clone();
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
        if (!force && self.stroke.is_some())
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
            self.error = Some(format!("Workspace/brush settings save failed: {e}"));
        }
    }

    #[cfg(test)]
    pub(crate) fn saved_preferences_json(&self) -> String {
        parfait_storage::load_preferences(&self.recovery_path.with_file_name("preferences.sqlite"))
            .unwrap()
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_preferences_cover_persisted_fields_and_ignore_session_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.persist_preferences(true);
        let changes: [fn(&mut Parfait); 12] = [
            |app| app.calibration.points.insert(1, [0.5, 0.8]),
            |app| app.workspace.left_width += 10.,
            |app| app.workspace.panels[0].collapsed = !app.workspace.panels[0].collapsed,
            |app| app.presets[0].favorite = true,
            |app| app.presets[0].name.push_str(" changed"),
            |app| app.presets[0].brush.size += 1.,
            |app| app.brush.size += 1.,
            |app| app.tip_name.push_str(" changed"),
            |app| app.pressure_tab = !app.pressure_tab,
            |app| app.brush_library.group = None,
            |app| app.brush_library.favorites_only = true,
            |app| {
                app.brush_details.properties =
                    serde_json::from_value(serde_json::json!({"visible":vec![false;9]})).unwrap()
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
    fn unchanged_preferences_skip_io_and_failed_or_invalid_edits_remain_retryable() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
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
        let restored = Parfait::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(restored.brush.size, 41.);
    }

    #[test]
    fn legacy_cache_upgrades_and_future_preferences_remain_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.persist_preferences(true);
        let mut json: serde_json::Value =
            serde_json::from_str(&app.saved_preferences_json()).unwrap();
        let path = dir.path().join("preferences.sqlite");
        json["version"] = serde_json::json!(1);
        parfait_storage::save_preferences(&path, &json.to_string()).unwrap();
        app.restore_preferences();
        app.persist_preferences(true);
        assert_eq!(app.preferences_saved.as_ref().unwrap().version, 4);
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
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(16, 16);
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
        let bytes = parfait_storage::load_preferences(&dir.path().join("preferences.sqlite"))
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
}
