use super::Command;
use eframe::egui::{self, Key};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    #[serde(serialize_with = "serialize_key", deserialize_with = "deserialize_key")]
    pub key: Key,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

fn serialize_key<S: Serializer>(key: &Key, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(key.name())
}
fn deserialize_key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Key, D::Error> {
    let name = String::deserialize(deserializer)?;
    if name.len() <= 32
        && let Some(key) = Key::from_name(&name)
    {
        return Ok(key);
    }
    Err(serde::de::Error::custom("Unsupported shortcut key"))
}

impl Chord {
    pub fn new(key: Key, primary: bool, shift: bool, alt: bool) -> Self {
        let key = if key == Key::Equals && shift {
            Key::Plus
        } else {
            key
        };
        Self {
            key,
            primary,
            shift: shift && key != Key::Plus,
            alt,
        }
    }
    pub fn from_event(key: Key, modifiers: egui::Modifiers) -> Self {
        Self::new(
            key,
            modifiers.command || modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
        )
    }
    pub fn validate(self) -> bool {
        self == Self::new(self.key, self.primary, self.shift, self.alt)
            && !matches!(
                self.key,
                Key::Space | Key::Escape | Key::Enter | Key::Copy | Key::Cut | Key::Paste
            )
            && !(self.primary && self.key == Key::Q)
            && !(self.alt && self.key == Key::F4)
    }
    pub fn label(self) -> String {
        let mut label = String::new();
        if self.primary {
            label.push_str(if cfg!(target_os = "macos") {
                "Cmd+"
            } else {
                "Ctrl+"
            });
        }
        if self.alt {
            label.push_str(if cfg!(target_os = "macos") {
                "Option+"
            } else {
                "Alt+"
            });
        }
        if self.shift {
            label.push_str("Shift+");
        }
        label.push_str(match self.key {
            Key::OpenBracket => "[",
            Key::CloseBracket => "]",
            Key::Equals => "=",
            Key::Minus => "-",
            Key::Plus => "Plus",
            _ => self.key.name(),
        });
        label
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Binding {
    command: Command,
    chord: Chord,
}

#[derive(Clone, Default)]
struct Hint {
    keys: String,
    summary: String,
    tooltip: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Keymap {
    bindings: Vec<Binding>,
    #[serde(skip)]
    hints: Vec<Hint>,
}
impl PartialEq for Keymap {
    fn eq(&self, other: &Self) -> bool {
        self.bindings == other.bindings
    }
}
impl Eq for Keymap {}

#[derive(Debug, PartialEq, Eq)]
pub enum AssignmentError {
    Reserved,
    Missing,
    Limit,
    Duplicate,
    Conflict,
}
impl AssignmentError {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Reserved => {
                "This key is reserved for navigation, dialogs, text or the operating system."
            }
            Self::Missing => "The selected assignment no longer exists.",
            Self::Limit => "Each command supports up to four shortcuts.",
            Self::Duplicate => "This shortcut is already assigned to the selected command.",
            Self::Conflict => {
                "This shortcut belongs to another command. Replace it explicitly or choose another key."
            }
        }
    }
}

impl Default for Keymap {
    fn default() -> Self {
        use Command::*;
        let primary = true;
        let mut bindings = Vec::new();
        for (command, key, primary, shift, alt) in [
            (NewDrawing, Key::N, primary, false, false),
            (OpenProject, Key::O, primary, false, false),
            (SaveProject, Key::S, primary, false, false),
            (SaveAs, Key::S, primary, true, false),
            (ExportPng, Key::E, primary, true, false),
            (Undo, Key::Z, primary, false, false),
            (SelectAll, Key::A, primary, false, false),
            (Deselect, Key::D, primary, false, false),
            (InvertSelection, Key::I, primary, true, false),
            (Redo, Key::Y, primary, false, false),
            (Redo, Key::Z, primary, true, false),
            (ClearEditingTarget, Key::Delete, false, false, false),
            (ClearEditingTarget, Key::Backspace, false, false, false),
            (NewLayer, Key::N, primary, true, false),
            (SelectLayerAbove, Key::CloseBracket, false, false, true),
            (SelectLayerBelow, Key::OpenBracket, false, false, true),
            (ZoomIn, Key::Plus, primary, false, false),
            (ZoomIn, Key::Equals, primary, false, false),
            (ZoomOut, Key::Minus, primary, false, false),
            (ActualSize, Key::Num0, primary, false, true),
            (FitCanvas, Key::Num0, primary, false, false),
            (ResetDisplay, Key::Num0, primary, true, false),
            (RotateLeft, Key::ArrowLeft, false, true, false),
            (RotateRight, Key::ArrowRight, false, true, false),
            (FlipHorizontal, Key::H, false, true, false),
            (FlipVertical, Key::V, false, true, false),
            (RulerBar, Key::R, primary, false, false),
            (PixelGrid, Key::G, primary, true, false),
            (DisplaySettings, Key::K, primary, false, false),
            (ShortcutSettings, Key::K, primary, true, true),
            (HidePalettes, Key::Tab, false, false, false),
            (ToolInk, Key::P, false, false, false),
            (ToolSketch, Key::K, false, false, false),
            (ToolPaint, Key::B, false, false, false),
            (ToolAirbrush, Key::A, false, false, false),
            (ToolEraser, Key::E, false, false, false),
            (ToolBlend, Key::J, false, false, false),
            (ToolHand, Key::H, false, false, false),
            (ToolPicker, Key::I, false, false, false),
            (BrushSmaller, Key::OpenBracket, false, false, false),
            (BrushLarger, Key::CloseBracket, false, false, false),
            (ShortcutGuide, Key::F1, false, false, false),
        ] {
            bindings.push(Binding {
                command,
                chord: Chord::new(key, primary, shift, alt),
            });
        }
        let mut keymap = Self {
            bindings,
            hints: Vec::new(),
        };
        keymap.refresh();
        keymap
    }
}

impl Keymap {
    pub fn preferences(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
            hints: Vec::new(),
        }
    }
    pub fn chords(&self, command: Command) -> impl Iterator<Item = Chord> + '_ {
        self.bindings
            .iter()
            .filter(move |binding| binding.command == command)
            .map(|binding| binding.chord)
    }
    pub fn hint(&self, command: Command) -> &str {
        self.hints
            .get(command as usize)
            .map_or("", |hint| hint.keys.as_str())
    }
    pub fn summary(&self, command: Command) -> &str {
        self.hints
            .get(command as usize)
            .map_or("", |hint| hint.summary.as_str())
    }
    pub fn tooltip(&self, command: Command) -> &str {
        self.hints
            .get(command as usize)
            .map_or(command.label(), |hint| hint.tooltip.as_str())
    }

    pub fn refresh(&mut self) {
        self.bindings.sort_by_key(|binding| {
            (
                binding.command as u8,
                binding.chord.key,
                binding.chord.primary,
                binding.chord.shift,
                binding.chord.alt,
            )
        });
        self.hints = Command::ALL
            .iter()
            .map(|&command| {
                let labels: Vec<_> = self.chords(command).map(Chord::label).collect();
                let summary = match labels.as_slice() {
                    [] => String::new(),
                    [only] => only.clone(),
                    [first, rest @ ..] => format!("{first} (+{})", rest.len()),
                };
                let keys = labels.join(" / ");
                let mut tooltip = if keys.is_empty() {
                    command.label().into()
                } else {
                    format!("{} · {keys}", command.label())
                };
                if command == Command::ToolHand {
                    tooltip.push_str("; Space temporarily");
                }
                if command == Command::ToolPicker {
                    tooltip.push_str("; Alt temporarily");
                }
                if command == Command::LayerAlphaLock {
                    tooltip.push_str("\n기존 픽셀의 투명도를 유지하며 색만 바꿉니다.\n지우개는 투명하게 지우는 대신 흰색으로 칠합니다.");
                }
                if command == Command::LayerClipping {
                    tooltip.push_str("\n아래 기준 레이어의 불투명한 영역에만 표시합니다.\n영역 밖의 원본 픽셀은 지우지 않습니다.");
                }
                if command == Command::LayerReference {
                    tooltip.push_str("\n선택한 레이어를 참조 대상으로 표시합니다.\n참조 채우기와 경계 넘침 방지는 아직 지원하지 않습니다.");
                }
                if command == Command::LayerDraft {
                    tooltip.push_str("\n캔버스에는 보이고 PNG 출력에서는 제외됩니다.\n작업 파일과 복구 기록에는 그대로 저장합니다.");
                }
                if command == Command::ShowMaskArea {
                    tooltip.push_str("\n활성 레이어의 숨긴 마스크 범위를 보라색으로 표시합니다.\n비활성 마스크도 진단하며, 저장·내보내기에는 포함하지 않습니다.\n다른 레이어와 재시작 시 표시 범위가 달라질 수 있는 세션 표시입니다.");
                }
                if command == Command::ClearEditingTarget {
                    tooltip.push_str("\n마스크 선택 중에는 활성 마스크를 모두 표시하도록 비웁니다.\n원본 선택 중에는 선택 레이어의 원본 픽셀을 비웁니다.");
                }
                if command == Command::ClearLayerMask {
                    tooltip.push_str("\n활성 마스크만 모두 표시 상태로 비웁니다. 원본 픽셀은 유지합니다.");
                }
                if command == Command::ConvertToDrawingColor {
                    tooltip.push_str("\n활성 레이어 원본 전체의 색을 현재 그리기색으로 바꿉니다.\n알파·마스크·다른 레이어는 유지하며 선택 영역 제한은 아직 지원하지 않습니다.");
                }
                if command == Command::BrightnessToOpacity {
                    tooltip.push_str("\n활성 원본 전체에서 흰색은 투명하게, 어두운 부분은 검정 선으로 변환합니다.\n기존 알파에 역휘도를 곱하며 마스크는 유지합니다. 선택 영역 제한은 아직 지원하지 않습니다.");
                }
                Hint {
                    keys,
                    summary,
                    tooltip,
                }
            })
            .collect();
    }

    pub fn validate(&self) -> bool {
        if self.bindings.len() > Command::ALL.len() * 4 {
            return false;
        }
        for (index, binding) in self.bindings.iter().enumerate() {
            if !binding.chord.validate() || self.chords(binding.command).count() > 4 {
                return false;
            }
            for previous in &self.bindings[..index] {
                if previous.chord == binding.chord
                    && (previous.command == binding.command
                        || !previous.command.is_tool()
                        || !binding.command.is_tool())
                {
                    return false;
                }
            }
        }
        true
    }

    pub fn owners(&self, chord: Chord) -> impl Iterator<Item = Command> + '_ {
        self.bindings
            .iter()
            .filter(move |binding| binding.chord == chord)
            .map(|binding| binding.command)
    }
    pub fn conflicts(&self, command: Command, chord: Chord) -> bool {
        self.owners(chord)
            .any(|owner| owner != command && !(owner.is_tool() && command.is_tool()))
    }
    pub fn assign(
        &mut self,
        command: Command,
        old: Option<Chord>,
        chord: Chord,
        replace: bool,
    ) -> Result<(), AssignmentError> {
        if !chord.validate() {
            return Err(AssignmentError::Reserved);
        }
        if old.is_some_and(|old| !self.chords(command).any(|existing| existing == old)) {
            return Err(AssignmentError::Missing);
        }
        if self
            .chords(command)
            .any(|existing| existing == chord && Some(existing) != old)
        {
            return Err(AssignmentError::Duplicate);
        }
        if old.is_none() && self.chords(command).count() >= 4 {
            return Err(AssignmentError::Limit);
        }
        if self.conflicts(command, chord) && !replace {
            return Err(AssignmentError::Conflict);
        }
        self.bindings.retain(|binding| {
            !(binding.command == command && Some(binding.chord) == old)
                && !(replace && binding.command != command && binding.chord == chord)
        });
        self.bindings.push(Binding { command, chord });
        self.refresh();
        Ok(())
    }
    pub fn remove(&mut self, command: Command, chord: Chord) {
        self.bindings
            .retain(|binding| binding.command != command || binding.chord != chord);
        self.refresh();
    }
    pub fn resolve(&self, chord: Chord, current: Option<Command>) -> Option<Command> {
        fn choose(
            commands: impl Iterator<Item = Command>,
            current: Option<Command>,
        ) -> Option<Command> {
            let mut first = None;
            let mut after_current = false;
            for command in commands {
                first.get_or_insert(command);
                if after_current {
                    return Some(command);
                }
                after_current = Some(command) == current;
            }
            first
        }
        choose(self.owners(chord), current).or_else(|| {
            let bare = Chord::new(chord.key, false, false, false);
            choose(
                self.owners(bare)
                    .filter(|command| command.allows_modifier_fallback()),
                current,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cache_and_serialization_stay_in_catalog_order() {
        let map = Keymap::default();
        assert!(map.validate());
        for (index, &command) in Command::ALL.iter().enumerate() {
            assert_eq!(command as usize, index);
            let keys = map
                .chords(command)
                .map(Chord::label)
                .collect::<Vec<_>>()
                .join(" / ");
            assert_eq!(map.hint(command), keys);
            assert!(map.tooltip(command).starts_with(command.label()));
        }
        let json = serde_json::to_string(&map.preferences()).unwrap();
        assert!(!json.contains("hints"));
        let mut restored: Keymap = serde_json::from_str(&json).unwrap();
        assert!(restored.validate());
        restored.refresh();
        assert!(restored == map);
        assert_eq!(restored.hint(Command::Redo), map.hint(Command::Redo));
    }

    #[test]
    fn conflicts_are_atomic_and_replacement_removes_previous_owner() {
        let mut map = Keymap::default();
        let before = map.clone();
        let undo = Chord::new(Key::Z, true, false, false);
        let redo = Chord::new(Key::Y, true, false, false);
        assert_eq!(
            map.assign(Command::Undo, Some(undo), redo, false),
            Err(AssignmentError::Conflict)
        );
        assert!(map == before);
        map.assign(Command::Undo, Some(undo), redo, true).unwrap();
        assert_eq!(map.resolve(redo, None), Some(Command::Undo));
        assert_eq!(map.resolve(undo, None), None);
        assert_eq!(map.chords(Command::Redo).count(), 1);
        assert!(map.validate());
    }

    #[test]
    fn shared_tools_cycle_without_restoring_removed_keys() {
        let mut map = Keymap::default();
        let pen = Chord::new(Key::P, false, false, false);
        let eraser = Chord::new(Key::E, false, false, false);
        map.assign(Command::ToolInk, Some(pen), eraser, false)
            .unwrap();
        assert_eq!(map.resolve(eraser, None), Some(Command::ToolInk));
        assert_eq!(
            map.resolve(eraser, Some(Command::ToolInk)),
            Some(Command::ToolEraser)
        );
        assert_eq!(
            map.resolve(eraser, Some(Command::ToolEraser)),
            Some(Command::ToolInk)
        );
        assert_eq!(map.resolve(pen, None), None);
        let modified = Chord::new(Key::E, true, false, true);
        assert_eq!(
            map.resolve(modified, Some(Command::ToolInk)),
            Some(Command::ToolEraser)
        );
        map.remove(Command::ToolInk, eraser);
        map.remove(Command::ToolEraser, eraser);
        assert_eq!(map.resolve(modified, None), None);
    }

    #[test]
    fn limits_reserved_keys_and_invalid_serialized_maps_are_rejected() {
        let mut map = Keymap::default();
        for key in [Key::F5, Key::F6, Key::F7, Key::F8] {
            map.assign(
                Command::Notices,
                None,
                Chord::new(key, false, false, false),
                false,
            )
            .unwrap();
        }
        assert_eq!(
            map.assign(
                Command::Notices,
                None,
                Chord::new(Key::F9, false, false, false),
                false
            ),
            Err(AssignmentError::Limit)
        );
        for key in [Key::Space, Key::Escape, Key::Enter] {
            assert_eq!(
                map.assign(
                    Command::Undo,
                    None,
                    Chord::new(key, false, false, false),
                    false
                ),
                Err(AssignmentError::Reserved)
            );
        }
        let mut invalid = map.clone();
        invalid.bindings.push(invalid.bindings[0]);
        assert!(!invalid.validate());
        assert!(serde_json::from_str::<Keymap>(r#"{"bindings":[{"command":"undo","chord":{"key":"unknown","primary":false,"shift":false,"alt":false}}]}"#).is_err());
        assert_eq!(
            Chord::new(Key::Equals, true, true, false),
            Chord::new(Key::Plus, true, false, false)
        );
        let empty: Keymap = serde_json::from_str(r#"{"bindings":[]}"#).unwrap();
        assert!(empty.validate());
        assert_eq!(
            empty.resolve(Chord::new(Key::B, false, false, false), None),
            None
        );
    }
}
