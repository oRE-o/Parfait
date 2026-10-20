use super::*;
pub(super) const BUTTONS: &[&str] = &[
    "South",
    "East",
    "North",
    "West",
    "LeftTrigger",
    "RightTrigger",
    "LeftTrigger2",
    "RightTrigger2",
    "Select",
    "Start",
    "Mode",
    "LeftThumb",
    "RightThumb",
    "DPadUp",
    "DPadDown",
    "DPadLeft",
    "DPadRight",
];
const ACTIONS: &[Command] = &[
    Command::Undo,
    Command::Redo,
    Command::BrushSmaller,
    Command::BrushLarger,
    Command::ZoomIn,
    Command::ZoomOut,
    Command::FitCanvas,
    Command::RotateLeft,
    Command::RotateRight,
    Command::ToolInk,
    Command::ToolSketch,
    Command::ToolPaint,
    Command::ToolAirbrush,
    Command::ToolEraser,
    Command::ToolBlend,
    Command::ToolHand,
    Command::ToolPicker,
    Command::ShowQuickMenu,
    Command::CycleTools,
];
impl ControllerOptions {
    pub(super) fn validate(&self) -> bool {
        let valid_id = |id: &str| {
            id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
        };
        self.registered.len() <= 16
            && self.bindings.len() <= 272
            && self.registered.iter().enumerate().all(|(i, id)| {
                valid_id(id) && !self.registered[..i].contains(id)
            })
            && self.bindings.iter().enumerate().all(|(i, b)| {
                self.registered.contains(&b.device)
                    && BUTTONS.contains(&b.button.as_str())
                    && ACTIONS.contains(&b.command)
                    && !self.bindings[..i]
                        .iter()
                        .any(|p| p.device == b.device && p.button == b.button)
            })
    }
}
fn uuid(gamepad: gilrs::Gamepad<'_>) -> String {
    gamepad.uuid().iter().map(|v| format!("{v:02x}")).collect()
}
impl Parfait {
    pub(super) fn ensure_controllers(&mut self) {
        if self.settings.gilrs.is_some() || self.settings.controller_failed {
            return;
        }
        match gilrs::Gilrs::new() {
            Ok(gilrs) => {
                self.settings.gilrs = Some(gilrs);
                self.settings.controller_error = None;
            }
            Err(error) => {
                self.settings.controller_failed = true;
                self.settings.controller_error = Some(error.to_string());
            }
        }
    }
    pub(crate) fn poll_controllers(&mut self, ctx: &egui::Context) {
        if !self.options.controllers.enabled && self.settings.dialog.is_none() {
            self.settings.gilrs = None;
            self.settings.controller_was_active = false;
            self.settings.controller_armed_at = None;
            return;
        }
        if self.options.controllers.enabled {
            self.ensure_controllers();
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        }
        let enabled = self.options.controllers.enabled
            && ctx.input(|i| i.focused)
            && !self.dialog_open()
            && !ctx.egui_wants_keyboard_input();
        if enabled && !self.settings.controller_was_active {
            self.settings.controller_armed_at =
                Some(std::time::SystemTime::now());
        }
        self.settings.controller_was_active = enabled;
        let mut commands = Vec::new();
        if let Some(gilrs) = &mut self.settings.gilrs {
            for _ in 0..32 {
                let Some(event) = gilrs.next_event() else {
                    break;
                };
                if !enabled {
                    continue;
                }
                if self
                    .settings
                    .controller_armed_at
                    .is_some_and(|time| event.time < time)
                {
                    continue;
                }
                if let gilrs::EventType::ButtonPressed(button, _) = event.event
                {
                    let Some(pad) = gilrs.connected_gamepad(event.id) else {
                        continue;
                    };
                    let device = uuid(pad);
                    let button = format!("{button:?}");
                    if let Some(binding) = self
                        .options
                        .controllers
                        .bindings
                        .iter()
                        .find(|b| b.device == device && b.button == button)
                    {
                        commands.push(binding.command);
                    }
                }
            }
        }
        for command in commands {
            if self.command_disabled_reason(command).is_none() {
                self.execute_command(command, ctx);
            }
        }
    }
    pub(super) fn controller_controls(
        &mut self,
        ui: &mut egui::Ui,
        options: &mut ControllerOptions,
        _ctx: &egui::Context,
    ) {
        ui.label("운영체제에 연결된 게임패드의 버튼을 그리기 명령에 연결합니다. 펜·태블릿의 압력은 장치 필압 설정에서 조절합니다.");
        ui.checkbox(&mut options.enabled, "등록한 컨트롤러의 버튼 입력 사용");
        if ui.button("연결 장치 새로고침").clicked() {
            self.settings.controller_failed = false;
            self.settings.gilrs = None;
            self.ensure_controllers();
        }
        if ui.button("운영체제 Bluetooth 검색 열기").clicked()
            && let Err(error) = open_bluetooth_settings()
        {
            self.settings.controller_error = Some(error.to_string());
        }
        if let Some(error) = &self.settings.controller_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let devices: Vec<_> =
            self.settings.gilrs.as_ref().map_or_else(Vec::new, |g| {
                g.gamepads()
                    .take(16)
                    .map(|(_, pad)| {
                        let name =
                            pad.name().chars().take(128).collect::<String>();
                        (uuid(pad), name)
                    })
                    .collect()
            });
        if devices.is_empty() {
            ui.label("연결된 게임패드가 없습니다. Bluetooth 또는 USB로 연결한 뒤 새로고침해 주세요.");
        }
        for (id, name) in devices {
            ui.separator();
            ui.label(name);
            if !options.registered.contains(&id) {
                if ui
                    .add_enabled(
                        options.registered.len() < 16,
                        egui::Button::new("이 장치 등록"),
                    )
                    .clicked()
                {
                    options.registered.push(id.clone());
                }
                continue;
            }
            ui.collapsing("버튼에 명령 할당", |ui| {
                for button in BUTTONS {
                    let mut selected = options
                        .bindings
                        .iter()
                        .find(|b| b.device == id && b.button == *button)
                        .map(|b| b.command);
                    let before = selected;
                    ui.horizontal(|ui| {
                        ui.label(*button);
                        egui::ComboBox::from_id_salt((&id, *button))
                            .selected_text(
                                selected.map_or("할당 없음", Command::label),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut selected,
                                    None,
                                    "할당 없음",
                                );
                                for command in ACTIONS {
                                    ui.selectable_value(
                                        &mut selected,
                                        Some(*command),
                                        command.label(),
                                    );
                                }
                            });
                    });
                    if selected != before {
                        options
                            .bindings
                            .retain(|b| b.device != id || b.button != *button);
                        if let Some(command) = selected {
                            options.bindings.push(ControllerBinding {
                                device: id.clone(),
                                button: (*button).into(),
                                command,
                            });
                        }
                    }
                }
            });
        }
    }
}
pub(super) fn open_bluetooth_settings() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:bluetooth")
            .spawn()?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-b", "com.apple.systempreferences"])
            .spawn()?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        for (program, args) in [
            ("blueman-manager", vec![]),
            ("gnome-control-center", vec!["bluetooth"]),
            ("kcmshell6", vec!["kcm_bluetooth"]),
        ] {
            match std::process::Command::new(program).args(args).spawn() {
                Ok(_) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        anyhow::bail!(
            "Bluetooth 설정 프로그램을 찾지 못했습니다. 운영체제에서 장치를 연결한 뒤 장치 목록을 새로고침해 주세요."
        )
    }
}
