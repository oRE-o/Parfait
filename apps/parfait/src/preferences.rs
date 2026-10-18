use crate::{app::Parfait, brushes::Preset, workspace::Workspace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Preferences {
    pub version: u32,
    pub workspace: Workspace,
    pub presets: Vec<Preset>,
    pub brush: parfait_core::Brush,
    pub tip_name: String,
    #[serde(default)]
    pub pressure_tab: bool,
}
impl Preferences {
    pub fn validate(&self) -> bool {
        self.version == 1
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
                self.workspace = pref.workspace;
                self.presets = pref.presets;
                self.brush = pref.brush;
                self.tip_name = pref.tip_name;
                self.pressure_tab = pref.pressure_tab;
                self.preferences_saved = json;
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
        if self.preferences_read_failed
            || (!force && self.preferences_tick.elapsed().as_secs_f32() < 1.)
        {
            return;
        }
        self.preferences_tick = std::time::Instant::now();
        let prefs = Preferences {
            version: 1,
            workspace: self.workspace.clone(),
            presets: self.presets.clone(),
            brush: self.brush.clone(),
            tip_name: self.tip_name.clone(),
            pressure_tab: self.pressure_tab,
        };
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(prefs.validate(), "Invalid preferences");
            let json = serde_json::to_string(&prefs)?;
            if json != self.preferences_saved {
                parfait_storage::save_preferences(
                    &self.recovery_path.with_file_name("preferences.sqlite"),
                    &json,
                )?;
                self.preferences_saved = json;
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(format!("Workspace/brush settings save failed: {e}"));
        }
    }
}
