//! Small local editors for persistent text and polyline vector layers.
use crate::{app::Parfait, document_session::SessionKey, shortcuts::Command};
use eframe::egui;
use parfait_core::{
    Affine, ContentNode, LayerContent, LayerId, TextContent, VectorContent,
    VectorPath,
};
use std::sync::{Arc, OnceLock};
#[cfg(test)]
mod tests;
pub(crate) struct NativeDialog {
    owner: SessionKey,
    revision: u64,
    target: Option<LayerId>,
    pub body: Arc<LayerContent>,
    name: String,
    preview: Option<egui::TextureHandle>,
    error: Option<String>,
}
fn default_font() -> Arc<Vec<u8>> {
    static FONT: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    FONT.get_or_init(|| {
        Arc::new(
            include_bytes!("../assets/pretendard/Pretendard-Regular.otf")
                .to_vec(),
        )
    })
    .clone()
}
impl Parfait {
    pub(crate) fn native_command_reason(
        &self,
        command: Command,
    ) -> Option<&'static str> {
        let doc = &self.documents.active_session.document;
        if matches!(command, Command::NewTextLayer | Command::NewVectorLayer) {
            return doc
                .layer_insertion_reason()
                .or_else(|| parfait_storage::layer_growth_error(doc, 128));
        }
        if command == Command::EditNativeLayer
            && doc.layers[doc.active].alpha_locked
        {
            return Some("투명 픽셀 잠금을 해제해 주세요.");
        }
        if doc.native_content(doc.active).is_none() {
            return Some("텍스트 또는 벡터 레이어를 선택해 주세요.");
        }
        (!doc.layer_ancestors_unlocked(doc.active))
            .then_some("레이어와 부모 폴더의 잠금을 해제해 주세요.")
    }
    pub(crate) fn execute_native_command(&mut self, command: Command) {
        if let Some(reason) = self.native_command_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        if command == Command::RasterizeNativeLayer {
            self.edit_named(command.label(), |doc| {
                doc.rasterize_content(doc.active);
            });
            return;
        }
        if !self.reserve_pixel_headroom(32 * 1024 * 1024) {
            self.notice = Some("내용 편집을 위한 메모리가 부족합니다.".into());
            return;
        }
        self.finish_layer_edit();
        let session = &self.documents.active_session;
        let doc = &session.document;
        let (target, body, name) = if command == Command::EditNativeLayer {
            let node = doc.native_content(doc.active).unwrap();
            (
                Some(node.layer),
                node.body().clone(),
                doc.layers[doc.active].name.clone(),
            )
        } else {
            let origin = [
                (doc.width as f32 * 0.1).min(40.),
                (doc.height as f32 * 0.1).min(40.),
            ];
            let matrix = Affine::translation(origin[0], origin[1]);
            let body = if command == Command::NewTextLayer {
                LayerContent::Text(TextContent {
                    text: "텍스트".into(),
                    font: default_font(),
                    size: 32.,
                    color: [
                        self.brush.color[0],
                        self.brush.color[1],
                        self.brush.color[2],
                        255,
                    ],
                    matrix,
                })
            } else {
                LayerContent::Vector(VectorContent {
                    paths: vec![VectorPath {
                        points: Arc::new(vec![
                            [0., 0.],
                            [doc.width as f32 * 0.5, doc.height as f32 * 0.5],
                        ]),
                        width: 6.,
                        color: [
                            self.brush.color[0],
                            self.brush.color[1],
                            self.brush.color[2],
                            255,
                        ],
                        closed: false,
                        fill: false,
                    }],
                    matrix,
                })
            };
            (
                None,
                Arc::new(body),
                if command == Command::NewTextLayer {
                    "텍스트"
                } else {
                    "벡터"
                }
                .into(),
            )
        };
        self.native_dialog = Some(NativeDialog {
            owner: session.key,
            revision: doc.revision,
            target,
            body,
            name,
            preview: None,
            error: None,
        });
    }
    fn apply_native(&mut self, draft: &NativeDialog) -> Result<bool, String> {
        let session = &self.documents.active_session;
        if session.key != draft.owner
            || session.document.revision != draft.revision
        {
            return Err("대상 그림이 변경되었습니다. 다시 열어 주세요.".into());
        }
        let doc = &session.document;
        let node = ContentNode::new(
            draft.target.unwrap_or(LayerId(doc.next_layer_id)),
            draft.body.as_ref().clone(),
        )?;
        let reservation = u64::from(doc.width) * u64::from(doc.height) * 16
            + node.logical_bytes() * 2
            + 16 * 1024 * 1024;
        if !self.reserve_pixel_headroom(reservation) {
            return Err(
                "편집 결과와 실행 취소를 위한 메모리가 부족합니다.".into()
            );
        }
        let mut next = self.documents.active_session.document.clone();
        let changed = if let Some(id) = draft.target {
            let index =
                next.layer_index(id).ok_or("대상 레이어가 없습니다.")?;
            let changed =
                next.replace_content(index, draft.body.as_ref().clone())?;
            if next.layers[index].name != draft.name {
                if draft.name.trim().is_empty()
                    || draft.name.chars().count() > 128
                    || draft.name.chars().any(char::is_control)
                {
                    return Err("레이어 이름을 확인해 주세요.".into());
                }
                next.layers[index].name = draft.name.clone();
                next.touch();
                true
            } else {
                changed
            }
        } else {
            next.create_content_layer(
                draft.body.as_ref().clone(),
                draft.name.clone(),
            )?;
            true
        };
        if !next.validate()
            || parfait_storage::snapshot_size(&next)
                .is_none_or(|n| n > 512 * 1024 * 1024)
        {
            return Err("프로젝트 저장 예산을 초과합니다.".into());
        }
        if self
            .documents
            .logical_bytes()
            .saturating_sub(
                self.documents.active_session.document.logical_pixel_bytes(),
            )
            .saturating_add(next.logical_pixel_bytes())
            > self.documents.logical_limit
        {
            return Err("열린 문서의 논리 메모리 예산을 초과합니다.".into());
        }
        if changed {
            self.edit_named("텍스트 / 벡터 편집", |doc| *doc = next);
        }
        Ok(changed)
    }
    pub(crate) fn native_content_ui(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.native_dialog.take() else {
            return;
        };
        let mut action = 0;
        let mut open = true;
        let command = if matches!(draft.body.as_ref(), LayerContent::Text(_)) {
            Command::NewTextLayer
        } else {
            Command::NewVectorLayer
        };
        let response=crate::editor_window::window(ctx,command,510.).open(&mut open).show(ctx,|ui|{
            ui.set_width(510.);
            ui.horizontal(|ui|{ui.label("이름");ui.text_edit_singleline(&mut draft.name);});
            let mut changed=false;
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-210.).clamp(140.,600.)).show(ui,|ui|{
                let body=Arc::make_mut(&mut draft.body);
                match body{
                    LayerContent::Text(t)=>{
                        changed|=ui.add(egui::TextEdit::multiline(&mut t.text).char_limit(128).desired_rows(3)).changed();
                        ui.horizontal(|ui|{
                            ui.label("크기");changed|=ui.add(egui::DragValue::new(&mut t.size).range(8. ..=512.)).changed();
                            changed|=ui.color_edit_button_srgba_unmultiplied(&mut t.color).changed();
                        });
                        ui.label("글꼴은 그림에 포함됩니다. 기본 한국어·라틴 문자와 줄바꿈을 지원합니다.");
                        if ui.button("로컬 TTF / OTF 글꼴…").clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("글꼴",&["ttf","otf"]).pick_file(){
                            let read=||->Result<Vec<u8>,String>{
                                let file=std::fs::File::open(path).map_err(|e|e.to_string())?;
                                use std::io::Read;
                                let mut bytes=Vec::new();file.take(16*1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
                                if bytes.len()>16*1024*1024||ttf_parser::Face::parse(&bytes,0).is_err(){return Err("16 MiB 이하의 유효한 글꼴을 선택해 주세요.".into());}Ok(bytes)
                            };
                            match read(){Ok(bytes)=>{t.font=Arc::new(bytes);changed=true;},Err(e)=>draft.error=Some(e)}
                        }
                    }
                    LayerContent::Vector(v)=>{
                        ui.label("경로의 점을 편집합니다. 좌표는 레이어 원점 기준입니다.");
                        let mut remove=None;
                        for (path_index,path) in v.paths.iter_mut().enumerate(){
                            egui::CollapsingHeader::new(format!("경로 {} ({}점)",path_index+1,path.points.len())).default_open(path_index==0).id_salt(path_index).show(ui,|ui|{
                                ui.horizontal(|ui|{
                                    changed|=ui.checkbox(&mut path.closed,"닫기").changed();
                                    changed|=ui.checkbox(&mut path.fill,"채우기").changed();
                                    ui.label("두께");changed|=ui.add(egui::DragValue::new(&mut path.width).range(0.1..=512.)).changed();
                                    changed|=ui.color_edit_button_srgba_unmultiplied(&mut path.color).changed();
                                    if ui.small_button("경로 삭제").clicked(){remove=Some(path_index);}
                                });
                                egui::ScrollArea::vertical().id_salt(("points",path_index)).max_height(170.).show_rows(ui,24.,path.points.len(),|ui,range|{
                                    for i in range{
                                        let mut point=path.points[i];ui.horizontal(|ui|{
                                            ui.label(format!("{}",i+1));
                                            let x=ui.add(egui::DragValue::new(&mut point[0]).range(-16384. ..=16384.)).changed();
                                            let y=ui.add(egui::DragValue::new(&mut point[1]).range(-16384. ..=16384.)).changed();
                                            if x||y{Arc::make_mut(&mut path.points)[i]=point;changed=true;}
                                        });
                                    }
                                });
                                ui.horizontal(|ui|{
                                    if ui.add_enabled(path.points.len()<1024,egui::Button::new("점 추가")).clicked(){let p=*path.points.last().unwrap_or(&[0.,0.]);Arc::make_mut(&mut path.points).push([p[0]+10.,p[1]+10.]);changed=true;}
                                    if ui.add_enabled(path.points.len()>2,egui::Button::new("마지막 점 삭제")).clicked(){Arc::make_mut(&mut path.points).pop();changed=true;}
                                });
                            });
                        }
                        if let Some(index)=remove{v.paths.remove(index);changed=true;}
                        if ui.add_enabled(v.paths.len()<64,egui::Button::new("경로 추가")).clicked(){v.paths.push(VectorPath{points:Arc::new(vec![[0.,0.],[64.,64.]]),width:6.,color:[0,0,0,255],closed:false,fill:false});changed=true;}
                    }
                }
                let matrix=match body{LayerContent::Text(t)=>&mut t.matrix,LayerContent::Vector(v)=>&mut v.matrix};
                ui.horizontal(|ui|{ui.label("위치 X / Y");for i in 4..6{changed|=ui.add(egui::DragValue::new(&mut matrix.0[i]).range(-16384. ..=16384.)).changed();}});
                if changed{draft.preview=None;}
                if let Some(texture)=&draft.preview{
                    let size=texture.size_vec2();let scale=(450./size.x).min(220./size.y).min(1.);
                    ui.image((texture.id(),size*scale));
                }
            });
            if let Some(error)=&draft.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            ui.separator();ui.horizontal(|ui|{
                if ui.button("미리 보기").clicked(){action=1;}
                if ui.button("적용").clicked(){action=2;}
                if ui.button("취소").clicked(){action=3;}
            });
        });
        let _ = response;
        if !open
            || (!egui::Popup::is_any_open(ctx)
                && ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                }))
        {
            return;
        }
        if action == 1 {
            let doc = &self.documents.active_session.document;
            let size = [doc.width as usize, doc.height as usize];
            let needed = u64::from(doc.width) * u64::from(doc.height) * 16
                + 16 * 1024 * 1024;
            if self.reserve_pixel_headroom(needed) {
                match ContentNode::new(LayerId(1), draft.body.as_ref().clone())
                    .and_then(|n| n.render(size[0] as u32, size[1] as u32))
                {
                    Ok(pixels) => {
                        draft.preview = Some(ctx.load_texture(
                            "native_preview",
                            egui::ColorImage::from_rgba_unmultiplied(
                                size, &pixels,
                            ),
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                    Err(e) => draft.error = Some(e.into()),
                }
            } else {
                draft.error =
                    Some("미리 보기를 위한 메모리가 부족합니다.".into());
            }
        }
        if action == 2 {
            match self.apply_native(&draft) {
                Ok(_) => return,
                Err(e) => draft.error = Some(e),
            }
        }
        if action != 3 {
            self.native_dialog = Some(draft);
        }
    }
}
