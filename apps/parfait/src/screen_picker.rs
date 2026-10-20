use crate::{app::Parfait, workspace::Location};
use eframe::egui;
use std::sync::{atomic::Ordering, mpsc};
#[cfg(test)]
mod tests;

struct CapturedScreen {
    monitors: Vec<(u32, String)>,
    selected: u32,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
pub(crate) struct ScreenPicker {
    receiver: Option<mpsc::Receiver<Result<CapturedScreen, String>>>,
    frame: Option<CapturedScreen>,
    texture: Option<egui::TextureHandle>,
    pub hidden: bool,
    hide_before_capture: bool,
    zoom: f32,
    error: Option<String>,
    sample: Option<[u8; 3]>,
}
impl Parfait {
    pub(crate) fn start_screen_picker(
        &mut self,
        hide: bool,
        monitor: Option<u32>,
        ctx: &egui::Context,
    ) {
        if self
            .screen_capture_busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            self.notice = Some("화면 캡처가 이미 진행 중입니다.".into());
            return;
        }
        if !self.reserve_pixel_headroom(256 * 1024 * 1024) {
            self.screen_capture_busy.store(false, Ordering::Release);
            self.notice =
                Some("화면 색 추출을 위한 메모리가 부족합니다.".into());
            return;
        }
        let palettes: Vec<_> = self
            .workspace
            .panels
            .iter()
            .filter(|p| p.visible && p.location == Location::Window)
            .map(|p| {
                egui::ViewportId::from_hash_of(("palette_window", p.palette))
            })
            .collect();
        if hide {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            for id in &palettes {
                ctx.send_viewport_cmd_to(
                    *id,
                    egui::ViewportCommand::Visible(false),
                );
            }
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let restore_ids = palettes.clone();
        let context = ctx.clone();
        let busy = self.screen_capture_busy.clone();
        let worker = std::thread::Builder::new()
            .name("parfait-screen-color".into())
            .spawn(move || {
                let _guard = CaptureGuard {
                    ctx: context.clone(),
                    palettes: palettes.clone(),
                    hidden: hide,
                    busy,
                };
                // Allow the native window manager to present the hidden/closed menu state.
                std::thread::sleep(std::time::Duration::from_millis(180));
                let result = capture(monitor).map_err(|e| e.to_string());
                let _ = sender.send(result);
            });
        if let Err(error) = worker {
            self.screen_capture_busy.store(false, Ordering::Release);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            for id in restore_ids {
                ctx.send_viewport_cmd_to(
                    id,
                    egui::ViewportCommand::Visible(true),
                );
            }
            self.error = Some(error.to_string());
            return;
        }
        self.screen_picker = Some(ScreenPicker {
            receiver: Some(receiver),
            frame: None,
            texture: None,
            hidden: hide,
            hide_before_capture: hide,
            zoom: 1.,
            error: None,
            sample: None,
        });
    }
    pub(crate) fn screen_picker_ui(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.screen_picker.take() else {
            return;
        };
        if let Some(receiver) = &picker.receiver {
            match receiver.try_recv() {
                Ok(Ok(frame)) => {
                    picker.texture = Some(ctx.load_texture(
                        "screen_color_capture",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.pixels,
                        ),
                        egui::TextureOptions::NEAREST,
                    ));
                    picker.frame = Some(frame);
                    picker.receiver = None;
                    picker.hidden = false;
                    ctx.request_repaint();
                }
                Ok(Err(error)) => {
                    picker.error = Some(error);
                    picker.receiver = None;
                    picker.hidden = false;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    picker.error =
                        Some("화면 캡처 작업이 종료되었습니다.".into());
                    picker.receiver = None;
                    picker.hidden = false;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut cancel = false;
        let mut chosen = None;
        let mut again = None;
        let response=egui::Modal::new(egui::Id::new("screen_color_picker")).show(ctx,|ui|{
            ui.set_width((ctx.content_rect().width()-80.).clamp(400.,900.));ui.heading("화면색 추출");
            if let Some(frame)=&picker.frame {
                let mut selected=frame.selected;
                egui::ComboBox::from_id_salt("screen_monitor").selected_text(frame.monitors.iter().find(|(id,_)|*id==selected).map_or("모니터",|(_,name)|name.as_str())).show_ui(ui,|ui|{for (id,name) in &frame.monitors{ui.selectable_value(&mut selected,*id,name);}});
                if selected!=frame.selected{again=Some(selected);}
                crate::widgets::numeric_slider(ui,&mut picker.zoom,0.5..=8.,"미리보기 확대",2,"×");
                let fit=(ui.available_width()/frame.width as f32).min((ctx.content_rect().height()-260.).max(100.)/frame.height as f32).min(1.);
                egui::ScrollArea::both().max_height((ctx.content_rect().height()-230.).max(100.)).show(ui,|ui|{
                    let size=egui::vec2(frame.width as f32,frame.height as f32)*fit*picker.zoom;
                    let response=ui.add(egui::Image::new(picker.texture.as_ref().unwrap()).fit_to_exact_size(size).sense(egui::Sense::click()));
                    if let Some(point)=response.hover_pos(){let uv=(point-response.rect.min)/response.rect.size();
                        let x=(uv.x*frame.width as f32).floor().clamp(0.,frame.width as f32-1.) as u32;let y=(uv.y*frame.height as f32).floor().clamp(0.,frame.height as f32-1.) as u32;
                        let i=(y*frame.width+x) as usize*4;picker.sample=Some(frame.pixels[i..i+3].try_into().unwrap());if response.clicked(){chosen=picker.sample;}
                    }
                });
                if let Some([r,g,b])=picker.sample{ui.colored_label(egui::Color32::from_rgb(r,g,b),format!("RGB {r}, {g}, {b} · #{r:02X}{g:02X}{b:02X}"));}
                ui.label("원하는 픽셀을 클릭하면 그리기색에 적용합니다. 캡처 이미지는 파일에 저장하지 않습니다.");
            }else if picker.receiver.is_some(){ui.spinner();ui.label("화면을 읽고 있습니다…");}
            if let Some(error)=&picker.error{ui.colored_label(egui::Color32::LIGHT_RED,error);ui.label("운영체제 권한이나 화면 캡처 지원 상태를 확인해 주세요.");}
            ui.separator();ui.horizontal(|ui|{if ui.add_enabled(picker.receiver.is_none(),egui::Button::new("다시 캡처")).clicked(){again=Some(picker.frame.as_ref().map_or(0,|f|f.selected));}cancel|=ui.button("취소").clicked();});
        });
        if response.is_top_modal && !response.any_popup_open {
            cancel |= ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
        }
        if let Some(color) = chosen {
            self.brush.color = color;
            return;
        }
        if cancel {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            return;
        }
        if let Some(monitor) = again {
            if self.screen_capture_busy.load(Ordering::Acquire) {
                self.screen_picker = Some(picker);
                ctx.request_repaint();
                return;
            }
            self.start_screen_picker(
                picker.hide_before_capture,
                (monitor != 0).then_some(monitor),
                ctx,
            );
            return;
        }
        self.screen_picker = Some(picker);
    }
}
fn capture(request: Option<u32>) -> anyhow::Result<CapturedScreen> {
    let monitors = xcap::Monitor::all()?;
    anyhow::ensure!(!monitors.is_empty(), "모니터를 찾지 못했습니다.");
    let monitor = request
        .and_then(|id| monitors.iter().find(|m| m.id().ok() == Some(id)))
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)))
        .unwrap_or(&monitors[0]);
    let (width, height) = (monitor.width()?, monitor.height()?);
    anyhow::ensure!(
        width > 0
            && height > 0
            && u64::from(width) * u64::from(height) <= 4096 * 4096,
        "화면은 최대 1677만 픽셀까지 읽을 수 있습니다."
    );
    let scale = if cfg!(target_os = "macos") {
        monitor.scale_factor()?.max(1.)
    } else {
        1.
    };
    anyhow::ensure!(
        width as f64 * height as f64 * f64::from(scale).powi(2)
            <= 4096. * 4096.,
        "화면의 실제 픽셀 수가 캡처 제한을 초과합니다."
    );
    let frame = monitor.capture_image()?;
    let (width, height) = (frame.width(), frame.height());
    anyhow::ensure!(
        width > 0
            && height > 0
            && u64::from(width) * u64::from(height) <= 4096 * 4096,
        "캡처 중 화면 크기가 제한을 초과했습니다."
    );
    let selected = monitor.id()?;
    let names = monitors
        .iter()
        .take(16)
        .filter_map(|m| {
            Some((
                m.id().ok()?,
                m.friendly_name()
                    .or_else(|_| m.name())
                    .ok()?
                    .chars()
                    .take(128)
                    .collect(),
            ))
        })
        .collect();
    Ok(CapturedScreen {
        monitors: names,
        selected,
        width,
        height,
        pixels: frame.into_raw(),
    })
}

struct CaptureGuard {
    ctx: egui::Context,
    palettes: Vec<egui::ViewportId>,
    hidden: bool,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        if self.hidden {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            for id in &self.palettes {
                self.ctx.send_viewport_cmd_to(
                    *id,
                    egui::ViewportCommand::Visible(true),
                );
            }
        }
        self.busy.store(false, Ordering::Release);
        self.ctx.request_repaint();
    }
}
