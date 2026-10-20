use super::*;
use crate::shortcuts::Command;
pub(super) struct TabDrag {
    id: DocumentId,
    target: usize,
    outside: bool,
}
impl Parfait {
    pub(crate) fn cycle_canvas(&mut self, backward: bool) {
        let Some(active) = self.documents.active_id() else {
            return;
        };
        let order = &self.documents.tab_order;
        let index = order.iter().position(|id| *id == active).unwrap();
        let next = if backward {
            (index + order.len() - 1) % order.len()
        } else {
            (index + 1) % order.len()
        };
        self.activate_document(order[next], true);
    }
    fn reorder_canvas(&mut self, id: DocumentId, boundary: usize) {
        if self.tab_switch_blocked() {
            return;
        }
        let Some(from) = self.documents.tab_order.iter().position(|n| *n == id)
        else {
            return;
        };
        let mut to = boundary.min(self.documents.tab_order.len());
        if to > from {
            to -= 1;
        }
        if to == from {
            return;
        }
        self.documents.tab_order.remove(from);
        self.documents.tab_order.insert(to, id);
        self.tabs.drawn_active = None;
        // The durable session layout observes this order on the next maintenance.
    }
    pub(crate) fn canvas_menu_entries(&mut self, ui: &mut egui::Ui) {
        let active = self.documents.active_id();
        if self.documents.tab_order.is_empty() {
            ui.add_enabled(false, egui::Button::new("열린 그림 없음"));
            return;
        }
        for id in self.documents.tab_order.clone() {
            let session = self.documents.get(id).unwrap();
            let title = format!(
                "{}{}",
                session.title(),
                if session.modified() { " *" } else { "" }
            );
            let response = ui.selectable_label(active == Some(id), title);
            #[cfg(test)]
            ui.data_mut(|d| {
                d.insert_temp(egui::Id::new(("canvas_menu", id)), response.rect)
            });
            if response.clicked() {
                self.activate_document(id, true);
                ui.close();
            }
        }
    }
    pub(crate) fn document_tabs_ui(&mut self, ui: &mut egui::Ui) {
        let active = self.documents.active_id();
        let reveal = active != self.tabs.drawn_active;
        let mut activate = None;
        let mut close = None;
        let mut reorder = None;
        let mut rects = Vec::new();
        let mut drag_started = None;
        let mut drag_stopped = false;
        let panel=egui::Panel::top("multi_document_tabs").frame(egui::Frame::NONE.fill(egui::Color32::from_gray(42)).inner_margin(egui::Margin::symmetric(3,2))).show(ui,|ui|{
            self.recovery_items_ui(ui);
            if let Some(message)=self.notice.clone(){ui.horizontal(|ui|{ui.add_sized([(ui.available_width()-25.).max(1.),18.],egui::Label::new(&message).truncate()).on_hover_text(message);if ui.small_button("×").on_hover_text("알림 닫기").clicked(){self.notice=None;}});}
            ui.horizontal(|ui|{
                let available=(ui.available_width()-30.).max(60.);
                ui.allocate_ui_with_layout(egui::vec2(available,26.),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                    egui::ScrollArea::horizontal().id_salt("canvas_tabs_scroll").auto_shrink([false,true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui,|ui|{
                        ui.spacing_mut().item_spacing.x=2.;
                        ui.horizontal(|ui|{
                            for id in self.documents.tab_order.clone(){
                                let session=self.documents.get(id).unwrap();let title=format!("{}{}",session.title(),if session.modified(){" *"}else{""});
                                let source=session.project.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"아직 파일로 저장하지 않은 그림".into());
                                ui.push_id(id,|ui|{
                                    let button=egui::Button::new(&title).truncate().selected(active==Some(id)).sense(egui::Sense::click_and_drag());
                                    let tab=ui.add_sized([140.,24.],button).on_hover_text(format!("{title}\n{source}\n드래그: 순서 변경 · 가운데 클릭: 닫기"));
                                    #[cfg(test)]ui.data_mut(|d|d.insert_temp(egui::Id::new(("tab",id)),tab.rect));
                                    if tab.clicked(){activate=Some(id);}
                                    if tab.clicked_by(egui::PointerButton::Middle){close=Some(id);}
                                    if tab.drag_started(){drag_started=Some(id);}
                                    drag_stopped|=tab.drag_stopped();
                                    if reveal && active==Some(id){tab.scroll_to_me(Some(egui::Align::Center));}
                                    tab.context_menu(|ui|{
                                        ui.label(&title);ui.separator();
                                        if ui.button("이 그림으로 전환").clicked(){activate=Some(id);ui.close();}
                                        let index=self.documents.tab_order.iter().position(|n|*n==id).unwrap();
                                        if ui.add_enabled(index>0,egui::Button::new("왼쪽으로 이동")).clicked(){reorder=Some((id,index-1));ui.close();}
                                        if ui.add_enabled(index+1<self.documents.tab_order.len(),egui::Button::new("오른쪽으로 이동")).clicked(){reorder=Some((id,index+2));ui.close();}
                                        if ui.button("닫기").clicked(){close=Some(id);ui.close();}
                                        ui.separator();ui.label(&source);
                                    });
                                    let exit=ui.add_sized([18.,24.],egui::Button::new("×").frame(false)).on_hover_text("그림 닫기");
                                    #[cfg(test)]ui.data_mut(|d|d.insert_temp(egui::Id::new(("tab_close",id)),exit.rect));
                                    if exit.clicked(){close=Some(id);}
                                    let r=tab.rect.union(exit.rect);rects.push((id,r));
                                    if active==Some(id){ui.painter().hline(r.x_range(),r.bottom(),egui::Stroke::new(2.,crate::theme::ACCENT));}
                                });
                            }
                            for(id,title)in self.tabs.pending.iter().map(|p|(p.id,p.path.file_name().unwrap_or_default().to_string_lossy().into_owned())).collect::<Vec<_>>(){ui.label(format!("불러오는 중: {title}"));if ui.small_button("취소").clicked(){self.cancel_project_open(id);}}
                        });
                    });
                });
                ui.menu_button("...",|ui|{self.top_menu_command(ui,Command::NextCanvas);self.top_menu_command(ui,Command::PreviousCanvas);ui.separator();self.canvas_menu_entries(ui);});
            });
        });
        #[cfg(test)]
        ui.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("canvas_tab_strip"),
                panel.response.rect,
            )
        });
        let blocked = self.tab_switch_blocked() || !ui.is_enabled();
        if blocked
            || !ui.input(|i| i.focused)
            || ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            })
        {
            self.tabs.drag = None;
        } else {
            if let Some(id) = drag_started {
                let target = self
                    .documents
                    .tab_order
                    .iter()
                    .position(|n| *n == id)
                    .unwrap();
                self.settle_tab_input();
                self.tabs.drag = Some(TabDrag { id, target, outside: false });
            }
            if let Some(drag) = &mut self.tabs.drag {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    drag.outside = pos.y < panel.response.rect.top() - 12.
                        || pos.y > panel.response.rect.bottom() + 12.;
                    let target = rects
                        .iter()
                        .position(|(_, r)| pos.x < r.center().x)
                        .unwrap_or(rects.len());
                    drag.target = target;
                    let x = rects
                        .get(target)
                        .map(|(_, r)| r.left())
                        .or_else(|| rects.last().map(|(_, r)| r.right()));
                    if let Some(x) = x {
                        let r = panel.response.rect;
                        ui.painter().vline(
                            x,
                            r.y_range(),
                            egui::Stroke::new(2., crate::theme::ACCENT),
                        );
                    }
                }
                if drag_stopped || !ui.input(|i| i.pointer.primary_down()) {
                    let drag = self.tabs.drag.take().unwrap();
                    if !drag.outside {
                        reorder = Some((drag.id, drag.target));
                    }
                } else {
                    ui.ctx().request_repaint();
                }
            }
        }
        if !blocked {
            if let Some((id, boundary)) = reorder {
                self.reorder_canvas(id, boundary);
            }
            if let Some(id) = activate {
                self.activate_document(id, true);
            }
            if let Some(id) = close {
                self.request_tab_close(id);
            }
        }
        self.tabs.drawn_active = if reorder.is_some() { None } else { active };
    }
}
