use crate::{
    app::Parfait, display::DisplaySettings, shortcuts::Command, widgets::Icon,
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
mod controllers;
mod preferences_ui;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Options {
    pub toolbar: Vec<Command>,
    pub quick: Vec<Command>,
    pub cycle: Vec<Command>,
    pub space_pan: bool,
    pub space_rotate: bool,
    pub space_zoom: bool,
    pub alt_picker: bool,
    pub layer_picker: bool,
    pub middle_pan: bool,
    pub clipboard_read: bool,
    pub clipboard_write: bool,
    pub autosave_delay_ms: u32,
    pub font_path: Option<PathBuf>,
    pub font_size: f32,
    pub controllers: ControllerOptions,
}
impl Default for Options {
    fn default() -> Self {
        use Command::*;
        Self {
            toolbar: vec![
                NewDrawing,
                OpenProject,
                SaveProject,
                Undo,
                Redo,
                FitCanvas,
            ],
            quick: vec![ToolInk, ToolEraser, ToolPicker, Undo, Redo, FitCanvas],
            cycle: vec![ToolInk, ToolSketch, ToolPaint, ToolEraser, ToolBlend],
            space_pan: true,
            space_rotate: true,
            space_zoom: true,
            alt_picker: true,
            layer_picker: true,
            middle_pan: true,
            clipboard_read: true,
            clipboard_write: true,
            autosave_delay_ms: 400,
            font_path: None,
            font_size: 12.,
            controllers: Default::default(),
        }
    }
}
impl Options {
    pub fn validate(&self) -> bool {
        let valid = |items: &[Command], limit: usize| {
            items.len() <= limit
                && items.iter().enumerate().all(|(i, c)| {
                    Command::ALL.contains(c) && !items[..i].contains(c)
                })
        };
        valid(&self.toolbar, 16)
            && valid(&self.quick, 16)
            && valid(&self.cycle, 16)
            && !self.cycle.is_empty()
            && self.cycle.iter().all(|c| c.is_tool())
            && self.font_size.is_finite()
            && (8. ..=24.).contains(&self.font_size)
            && (100..=2000).contains(&self.autosave_delay_ms)
            && self
                .font_path
                .as_ref()
                .is_none_or(|p| p.as_os_str().len() <= 4096)
            && self.controllers.validate()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ControllerOptions {
    pub enabled: bool,
    pub registered: Vec<String>,
    pub bindings: Vec<ControllerBinding>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ControllerBinding {
    pub device: String,
    pub button: String,
    pub command: Command,
}

pub(crate) struct SettingsDialog {
    command: Command,
    candidate: Options,
    display: DisplaySettings,
    filter: String,
    error: Option<String>,
    page: preferences_ui::Page,
    workspace: crate::workspace::Workspace,
    calibration: parfait_core::PressureCalibration,
    pressure_point: Option<usize>,
}
#[derive(Default)]
pub(crate) struct SettingsState {
    pub dialog: Option<SettingsDialog>,
    pub quick_open: bool,
    pub about_open: bool,
    gilrs: Option<gilrs::Gilrs>,
    controller_failed: bool,
    controller_error: Option<String>,
    controller_was_active: bool,
    controller_armed_at: Option<std::time::SystemTime>,
}
impl Parfait {
    pub(crate) fn apply_font_options(
        &mut self,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        apply_font(ctx, &self.options)
    }
    pub(crate) fn execute_settings(
        &mut self,
        command: Command,
        ctx: &egui::Context,
    ) {
        if command == Command::ShowQuickMenu {
            self.settings.quick_open = true;
            return;
        }
        if command == Command::CycleTools {
            let current = self.active_tool_command();
            let next = self
                .options
                .cycle
                .iter()
                .position(|c| Some(*c) == current)
                .map_or(0, |i| (i + 1) % self.options.cycle.len());
            self.execute_command(self.options.cycle[next], ctx);
            return;
        }
        if command == Command::EditWirelessSearch {
            if let Err(error) = controllers::open_bluetooth_settings() {
                self.error = Some(error.to_string());
            }
            return;
        }
        if matches!(
            command,
            Command::EditControllerSettings
                | Command::EditControllerAdd
                | Command::EditExternalDevice
        ) {
            self.ensure_controllers();
        }
        self.settings.dialog = Some(SettingsDialog {
            command,
            candidate: self.options.clone(),
            display: self.display,
            filter: String::new(),
            error: None,
            page: preferences_ui::Page::Interface,
            workspace: self.workspace.clone(),
            calibration: self.calibration.clone(),
            pressure_point: None,
        });
    }
    pub(crate) fn settings_ui(&mut self, ctx: &egui::Context) {
        if self.settings.about_open {
            let mut open = true;
            let mut close = false;
            egui::Window::new("Parfait에 대하여").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
                ui.heading("Parfait");
                ui.label(format!("버전 {}", env!("CARGO_PKG_VERSION")));
                ui.label("가볍고 반응 빠른 로컬 드로잉 도구");
                ui.label("그리기와 복구 저장은 계정 없이 오프라인으로 동작합니다.");
                ui.separator();
                if ui.button("오픈소스 라이선스…").clicked() { self.show_licenses = true; close = true; }
                close |= ui.button("닫기").clicked();
            });
            self.settings.about_open = open
                && !close
                && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
            return;
        }
        if self.settings.quick_open {
            let mut chosen = None;
            let mut close = false;
            egui::Modal::new(egui::Id::new("quick_menu")).show(ctx, |ui| {
                ui.heading("퀵 메뉴");
                egui::Grid::new("quick_commands").show(ui, |ui| {
                    for (i, command) in self.options.quick.iter().enumerate() {
                        // The quick menu itself owns the modal; inspect command state after it closes.
                        if ui.button(command.label()).clicked() {
                            chosen = Some(*command);
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
                close |= ui.button("닫기").clicked();
            });
            close |= ctx.input(|i| i.key_pressed(egui::Key::Escape));
            if close || chosen.is_some() {
                self.settings.quick_open = false;
            }
            if let Some(command) = chosen {
                self.execute_command(command, ctx);
            }
            return;
        }
        let Some(mut dialog) = self.settings.dialog.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        let mut open = true;
        let width = if dialog.command == Command::EditPreferences {
            800.
        } else {
            480.
        };
        let response=crate::editor_window::window(ctx,dialog.command,width).open(&mut open).show(ctx,|ui|{
            ui.set_width(width);
            egui::ScrollArea::vertical().max_height((ui.clip_rect().bottom()-ui.cursor().top()-60.).clamp(120.,620.)).show(ui,|ui|{
                use Command::*;
                match dialog.command {
                    EditPreferences=>self.preferences_contents(ui,&mut dialog),
                    EditModifierSettings=>{
                        for (value,label) in [(&mut dialog.candidate.space_pan,"Space + 드래그: 이동"),(&mut dialog.candidate.space_rotate,"Shift + Space + 드래그: 보기 회전"),(&mut dialog.candidate.space_zoom,"Ctrl/Cmd + Space + 드래그: 보기 확대"),(&mut dialog.candidate.alt_picker,"Alt + 클릭: 캔버스 색 추출"),(&mut dialog.candidate.layer_picker,"Ctrl/Cmd + Shift + 클릭: 레이어 선택"),(&mut dialog.candidate.middle_pan,"마우스 가운데 버튼: 이동")] {ui.checkbox(value,label);}
                    }
                    EditCommandBarSettings|EditQuickSettings|EditToolCycleSettings=>{
                        ui.label("등록된 순서대로 표시하거나 순환합니다. 행의 화살표로 순서를 바꿀 수 있습니다.");
                        let items=if dialog.command==EditCommandBarSettings{&mut dialog.candidate.toolbar}else if dialog.command==EditQuickSettings{&mut dialog.candidate.quick}else{&mut dialog.candidate.cycle};
                        command_list(ui,items,&mut dialog.filter,dialog.command==EditToolCycleSettings);
                        ui.label(if dialog.command==EditQuickSettings{format!("열기 단축키: {}",self.keymap.hint(ShowQuickMenu))}else if dialog.command==EditToolCycleSettings{format!("순환 단축키: {}",self.keymap.hint(CycleTools))}else{String::new()});
                    }
                    EditFontSettings=>{
                        ui.label("기본 UI 글꼴은 Pretendard입니다. 다른 TTF/OTF 파일을 선택하면 기본 글꼴 앞에 추가합니다.");
                        ui.label(dialog.candidate.font_path.as_ref().map_or("기본 글꼴".into(),|p|p.display().to_string()));
                        if ui.button("글꼴 파일 추가 / 선택…").clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("OpenType", &["ttf","otf"]).pick_file(){dialog.candidate.font_path=Some(path);}
                        if ui.button("기본 글꼴로 복원").clicked(){dialog.candidate.font_path=None;}
                        crate::widgets::numeric_slider(ui,&mut dialog.candidate.font_size,8. ..=24.,"UI 글꼴 크기",1,"");
                    }
                    EditPrivacySettings=>{
                        ui.label("클립보드 이미지·SVG 읽기와 쓰기를 허용할지 정합니다. 계정이나 원격 전송 서비스는 사용하지 않습니다.");
                        ui.checkbox(&mut dialog.candidate.clipboard_read,"운영체제 클립보드에서 읽기 허용");ui.checkbox(&mut dialog.candidate.clipboard_write,"운영체제 클립보드에 쓰기 허용");
                    }
                    EditSystemServices=>{
                        ui.label("로컬 복구 저장 지연을 조절합니다. 종료 시 강제 저장과 오류 후 재시도는 유지됩니다.");
                        crate::widgets::numeric_slider(ui,&mut dialog.candidate.autosave_delay_ms,100..=2000,"복구 제출 지연",0," ms");
                        ui.label(format!("복구 위치: {}",self.recovery_path.parent().unwrap().display()));ui.label("저장 오류 재시도: 3초");
                    }
                    EditControllerSettings|EditControllerAdd|EditExternalDevice=>self.controller_controls(ui,&mut dialog.candidate.controllers,ctx),
                    _=>{}
                }
                if let Some(error)=&dialog.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            });
            ui.separator();crate::widgets::action_row(ui,|ui|{let no=crate::widgets::action_button(ui,"취소",80.);let ok=crate::widgets::action_button(ui,"확인",80.);
                #[cfg(test)]ui.data_mut(|d|{d.insert_temp(egui::Id::new("settings_accept"),ok.rect);d.insert_temp(egui::Id::new("settings_cancel"),no.rect);});
                accept|=ok.clicked();cancel|=no.clicked();});
        });
        let _ = response;
        cancel |= !open;
        if !egui::Popup::is_any_open(ctx) {
            cancel |= ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
        }
        if cancel {
            return;
        }
        if accept {
            if !dialog.candidate.validate()
                || !dialog.display.validate()
                || !dialog.calibration.validate()
                || !dialog.workspace.validate()
            {
                dialog.error =
                    Some("설정 범위나 명령 목록을 확인해 주세요.".into());
            } else {
                if self.preferences_read_failed {
                    dialog.error=Some("기존 설정 파일을 읽지 못해 보호 중입니다. 설정을 적용하지 않았습니다.".into());
                    self.settings.dialog = Some(dialog);
                    return;
                }
                let previous_options = self.options.clone();
                let previous_display = self.display;
                let previous_calibration = self.calibration.clone();
                let previous_workspace = self.workspace.clone();
                let previous_fonts = ctx.fonts(|f| f.definitions().clone());
                let previous_style = ctx.style_of(egui::Theme::Dark);
                let font_changed = self.options.font_path
                    != dialog.candidate.font_path
                    || self.options.font_size != dialog.candidate.font_size;
                let result = if font_changed {
                    apply_font(ctx, &dialog.candidate)
                } else {
                    Ok(())
                };
                if let Err(error) = result {
                    dialog.error = Some(error.to_string());
                } else {
                    self.options = dialog.candidate.clone();
                    self.display = dialog.display;
                    if dialog.command == Command::EditPreferences {
                        self.workspace = dialog.workspace.clone();
                        self.calibration = dialog.calibration.clone();
                    }
                    self.persist_preferences(true);
                    if self
                        .preferences_saved
                        .as_ref()
                        .is_some_and(|saved| saved.matches(self))
                    {
                        return;
                    }
                    self.options = previous_options;
                    self.display = previous_display;
                    self.calibration = previous_calibration;
                    self.workspace = previous_workspace;
                    if font_changed {
                        ctx.set_fonts(previous_fonts);
                        ctx.set_style_of(egui::Theme::Dark, previous_style);
                    }
                    dialog.error=Some(self.error.take().unwrap_or_else(||"설정을 저장하지 못했습니다. 이전 값은 유지되며 다시 시도할 수 있습니다.".into()));
                }
            }
        }
        self.settings.dialog = Some(dialog);
    }
    pub(crate) fn configured_command_bar(&mut self, ui: &mut egui::Ui) {
        let commands = self.options.toolbar.clone();
        for command in commands {
            let icon = match command {
                Command::NewDrawing => Some(Icon::Plus),
                Command::OpenProject => Some(Icon::Folder),
                Command::SaveProject | Command::SaveAs => Some(Icon::Save),
                Command::Undo => Some(Icon::Undo),
                Command::Redo => Some(Icon::Redo),
                Command::FitCanvas => Some(Icon::Fit),
                _ => None,
            };
            if let Some(icon) = icon {
                self.command_icon(ui, icon, command);
            } else {
                let label: String = command.label().chars().take(6).collect();
                let response = ui
                    .add_enabled(
                        self.command_disabled_reason(command).is_none(),
                        egui::Button::new(label),
                    )
                    .on_hover_text(self.keymap.tooltip(command));
                if response.clicked() {
                    self.execute_command(command, ui.ctx());
                }
            }
            if matches!(command, Command::SaveProject | Command::Redo) {
                ui.separator();
            }
        }
    }
}
fn apply_font(ctx: &egui::Context, options: &Options) -> anyhow::Result<()> {
    let mut fonts = crate::theme::font_definitions();
    if let Some(path) = &options.font_path {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "글꼴은 16 MiB 이하여야 합니다."
        );
        ttf_parser::Face::parse(&bytes, 0)
            .map_err(|e| anyhow::anyhow!("글꼴을 읽지 못했습니다: {e:?}"))?;
        fonts.font_data.insert(
            "custom_ui".into(),
            egui::FontData::from_owned(bytes).into(),
        );
        for family in
            [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
        {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "custom_ui".into());
        }
    }
    ctx.set_fonts(fonts);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        for text in [
            egui::TextStyle::Body,
            egui::TextStyle::Button,
            egui::TextStyle::Monospace,
        ] {
            style
                .text_styles
                .insert(text, egui::FontId::proportional(options.font_size));
        }
    });
    Ok(())
}
fn command_list(
    ui: &mut egui::Ui,
    items: &mut Vec<Command>,
    filter: &mut String,
    tools: bool,
) {
    let mut move_row = None;
    let mut remove = None;
    for (index, command) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(command.label());
            if ui.small_button("↑").clicked() && index > 0 {
                move_row = Some((index, index - 1));
            }
            if ui.small_button("↓").clicked() && index + 1 < items.len() {
                move_row = Some((index, index + 1));
            }
            if ui.small_button("×").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some((a, b)) = move_row {
        items.swap(a, b);
    }
    if let Some(i) = remove {
        items.remove(i);
    }
    ui.separator();
    ui.add(egui::TextEdit::singleline(filter).hint_text("추가할 명령 검색"));
    for command in Command::ALL.iter().filter(|c| {
        (!tools || c.is_tool())
            && c.label().to_lowercase().contains(&filter.to_lowercase())
    }) {
        if !items.contains(command)
            && ui
                .add_enabled(
                    items.len() < 16,
                    egui::Button::new(format!("+ {}", command.label())),
                )
                .clicked()
        {
            items.push(*command);
        }
    }
}
