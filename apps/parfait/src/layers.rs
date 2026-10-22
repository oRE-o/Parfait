use crate::{
    app::Parfait, editing::LayerEditKind, shortcuts::Command, widgets::Icon,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense};
use parfait_core::LayerId;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct LayerList {
    mask: u64,
    active: LayerId,
    ids: Vec<LayerId>,
    anchor: LayerId,
    drag: Option<LayerDrag>,
    owned: bool,
    rename: Option<LayerId>,
    rename_focus: bool,
    solo: Option<Vec<(LayerId, bool)>>,
    collapsed: Vec<LayerId>,
}
struct LayerDrag {
    mask: u64,
    revision: u64,
    origin: Pos2,
    moved: bool,
    collapse: Option<usize>,
}
#[derive(Clone, Copy)]
enum LayerDrop {
    Move { boundary: usize, parent: Option<LayerId>, y: f32 },
    Inside { folder: usize, rect: Rect },
    Group { target: usize, rect: Rect },
}
impl LayerList {
    pub(crate) fn has_name_editor(&self) -> bool {
        self.rename.is_some()
    }
    pub(crate) fn cancel_pointer(&mut self) {
        self.drag = None;
        self.owned = false;
        self.rename = None;
        self.rename_focus = false;
    }
    pub(crate) fn owns_pointer(&self) -> bool {
        self.owned || self.drag.is_some()
    }
}

impl crate::document_session::DocumentSession {
    pub(crate) fn selected_layer_mask(&self) -> u64 {
        let active = 1 << self.document.active;
        if self.layer_ui.list.active
            != self.document.layers[self.document.active].id
        {
            return active;
        }
        if self.layer_ui.list.ids.iter().copied().eq(self
            .document
            .layers
            .iter()
            .map(|layer| layer.id))
        {
            return self.layer_ui.list.mask | active;
        }
        self.document.layers.iter().enumerate().fold(
            active,
            |mask, (index, layer)| {
                if self.layer_ui.list.ids.iter().enumerate().any(
                    |(previous, id)| {
                        *id == layer.id
                            && self.layer_ui.list.mask & (1 << previous) != 0
                    },
                ) {
                    mask | (1 << index)
                } else {
                    mask
                }
            },
        )
    }
}

impl crate::document_session::DocumentSession {
    pub(crate) fn reset_layer_selection_view(&mut self) {
        let collapsed = std::mem::take(&mut self.layer_ui.list.collapsed);
        self.layer_ui.list = LayerList { collapsed, ..Default::default() };
    }

    pub(crate) fn reveal_layer(&mut self, index: usize) {
        let mut parent =
            self.document.layers.get(index).and_then(|layer| layer.parent);
        while let Some(id) = parent {
            self.layer_ui.list.collapsed.retain(|collapsed| *collapsed != id);
            parent = self
                .document
                .layer_index(id)
                .and_then(|index| self.document.layers[index].parent);
        }
    }
}

impl Parfait {
    pub(crate) fn reveal_layer(&mut self, index: usize) {
        let session = &mut *self.documents.active_session;
        session.reveal_layer(index);
    }

    fn layer_depth_and_hidden(&self, index: usize) -> (usize, bool) {
        let session = &*self.documents.active_session;
        let mut parent = session.document.layers[index].parent;
        let mut depth = 0;
        let mut hidden = false;
        while let Some(id) = parent {
            depth += 1;
            hidden |= session.layer_ui.list.collapsed.contains(&id);
            parent = session
                .document
                .layer_index(id)
                .and_then(|index| session.document.layers[index].parent);
        }
        (depth, hidden)
    }

    fn toggle_folder_view(&mut self, index: usize, modifiers: egui::Modifiers) {
        let id = self.documents.active_session.document.layers[index].id;
        let close = !self
            .documents
            .active_session
            .layer_ui
            .list
            .collapsed
            .contains(&id);
        let mut folders = 1 << index;
        if modifiers.command || modifiers.ctrl {
            folders = self
                .documents
                .active_session
                .document
                .layers
                .iter()
                .enumerate()
                .filter(|(_, layer)| {
                    layer.kind == parfait_core::LayerKind::Folder
                        && layer.parent
                            == self.documents.active_session.document.layers
                                [index]
                                .parent
                })
                .fold(0, |mask, (index, _)| mask | (1 << index));
        }
        if modifiers.alt {
            folders = self
                .documents
                .active_session
                .document
                .layer_selection_subtrees(folders)
                .unwrap_or(folders);
        }
        let session = &mut *self.documents.active_session;
        for (index, layer) in session.document.layers.iter().enumerate() {
            if folders & (1 << index) != 0
                && layer.kind == parfait_core::LayerKind::Folder
            {
                session.layer_ui.list.collapsed.retain(|id| *id != layer.id);
                if close {
                    session.layer_ui.list.collapsed.push(layer.id);
                }
            }
        }
        let mut parent = Some(
            self.documents.active_session.document.layers
                [self.documents.active_session.document.active]
                .id,
        );
        let mut fallback = self.documents.active_session.document.active;
        while let Some(id) = parent {
            let Some(index) =
                self.documents.active_session.document.layer_index(id)
            else {
                break;
            };
            if self
                .documents
                .active_session
                .layer_ui
                .list
                .collapsed
                .contains(&id)
            {
                fallback = index;
            }
            parent =
                self.documents.active_session.document.layers[index].parent;
        }
        if self
            .layer_depth_and_hidden(
                self.documents.active_session.document.active,
            )
            .1
        {
            self.layer_click(fallback, Default::default(), false);
        }
        self.documents.active_session.layer_ui.list.owned = true;
    }

    pub(crate) fn execute_selected_layer_command(&mut self, command: Command) {
        let mask = self.selected_layer_mask();
        if self.documents.active_session.document.has_non_raster_layers()
            && matches!(
                command,
                Command::DeleteLayer
                    | Command::MoveLayerUp
                    | Command::MoveLayerDown
            )
        {
            self.execute_tree_collection(command, mask);
            return;
        }
        if mask.count_ones() == 1 {
            self.execute_layer_command(
                command,
                self.documents.active_session.document.active,
            );
            return;
        }
        if self.command_disabled_reason(command).is_some() {
            return;
        }
        match command {
            Command::MoveLayerUp | Command::MoveLayerDown => {
                let mut order: Vec<_> =
                    (0..self.documents.active_session.document.layers.len())
                        .collect();
                let chosen = |index: usize| mask & (1u64 << index) != 0;
                if command == Command::MoveLayerUp {
                    for index in (0..order.len() - 1).rev() {
                        if chosen(order[index]) && !chosen(order[index + 1]) {
                            order.swap(index, index + 1);
                        }
                    }
                } else {
                    for index in 1..order.len() {
                        if chosen(order[index]) && !chosen(order[index - 1]) {
                            order.swap(index, index - 1);
                        }
                    }
                }
                self.apply_layer_order(order, mask);
            }
            Command::DeleteLayer => {
                let active = self.documents.active_session.document.active;
                self.edit_named("Delete layers", |document| {
                    let mut index = 0;
                    document.layers.retain(|_| {
                        let keep = mask & (1 << index) == 0;
                        index += 1;
                        keep
                    });
                    document.active = (0..active)
                        .filter(|index| mask & (1 << index) == 0)
                        .count()
                        .min(document.layers.len() - 1);
                });
                self.documents.active_session.layer_ui.list =
                    Default::default();
            }
            Command::LayerVisibility
            | Command::LayerLock
            | Command::LayerAlphaLock
            | Command::LayerReference
            | Command::LayerDraft
            | Command::LayerClipping => {
                let value = if command == Command::LayerLock {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .locked
                } else if command == Command::LayerAlphaLock {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .alpha_locked
                } else if command == Command::LayerClipping {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .clipped
                } else if command == Command::LayerReference {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .reference
                } else if command == Command::LayerDraft {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .draft
                } else {
                    !self.documents.active_session.document.layers
                        [self.documents.active_session.document.active]
                        .visible
                };
                self.edit_named(command.label(), |document| {
                    for (index, layer) in document.layers.iter_mut().enumerate()
                    {
                        if mask & (1 << index) != 0 {
                            if command == Command::LayerLock {
                                layer.locked = value;
                            } else if command == Command::LayerAlphaLock {
                                layer.alpha_locked = value;
                            } else if command == Command::LayerClipping {
                                layer.clipped = value;
                            } else if command == Command::LayerReference {
                                layer.reference = value;
                            } else if command == Command::LayerDraft {
                                layer.draft = value;
                            } else {
                                layer.visible = value;
                            }
                        }
                    }
                });
                self.documents.active_session.layer_ui.list.solo = None;
            }
            _ => {}
        }
    }
    pub(crate) fn duplicate_selected_layers(&mut self) {
        if self.documents.active_session.document.has_non_raster_layers() {
            self.execute_tree_collection(
                Command::DuplicateLayer,
                self.selected_layer_mask(),
            );
            return;
        }
        let mask = self.selected_layer_mask();
        if mask.count_ones() == 1 {
            self.duplicate_layer();
            return;
        }
        if self.command_disabled_reason(Command::DuplicateLayer).is_some() {
            return;
        }
        let mut selected = 0;
        self.edit_named("Duplicate layers", |document| {
            let active = document.active;
            let mut layers = Vec::with_capacity(
                document.layers.len() + mask.count_ones() as usize,
            );
            for (index, layer) in
                std::mem::take(&mut document.layers).into_iter().enumerate()
            {
                let copy = (mask & (1 << index) != 0).then(|| {
                    let mut copy = layer.clone();
                    copy.id = document.allocate_layer_id().unwrap();
                    if let Some(color) = document.layer_color(layer.id) {
                        document.layer_colors.push((copy.id, color));
                    }
                    copy.name.push_str(" copy");
                    copy
                });
                layers.push(layer);
                if let Some(copy) = copy {
                    if index == active {
                        document.active = layers.len();
                    }
                    selected |= 1 << layers.len();
                    layers.push(copy);
                }
            }
            document.layers = layers;
            document.layer_colors.sort_by_key(|(id, _)| id.0);
        });
        self.set_layer_selection(
            selected,
            self.documents.active_session.document.active,
        );
        self.documents.active_session.layer_ui.list.solo = None;
    }
    pub(crate) fn selected_layer_mask(&self) -> u64 {
        let session = &*self.documents.active_session;
        session.selected_layer_mask()
    }
    pub(crate) fn set_layer_selection(&mut self, mask: u64, anchor: usize) {
        let session = &mut *self.documents.active_session;
        session.layer_ui.list.mask = (mask
            & (u64::MAX >> (64 - session.document.layers.len())))
            | (1 << session.document.active);
        session.layer_ui.list.active =
            session.document.layers[session.document.active].id;
        session.layer_ui.list.ids.clear();
        session
            .layer_ui
            .list
            .ids
            .extend(session.document.layers.iter().map(|layer| layer.id));
        session.layer_ui.list.anchor = session.document.layers
            [anchor.min(session.document.layers.len() - 1)]
        .id;
    }
    pub(crate) fn layer_click(
        &mut self,
        layer: usize,
        modifiers: egui::Modifiers,
        check: bool,
    ) {
        if layer >= self.documents.active_session.document.layers.len() {
            return;
        }
        let mut mask = self.selected_layer_mask();
        let anchor = self
            .documents
            .active_session
            .document
            .layer_index(self.documents.active_session.layer_ui.list.anchor)
            .unwrap_or(self.documents.active_session.document.active);
        let mut active = layer;
        if modifiers.shift && !check {
            mask = (anchor.min(layer)..=anchor.max(layer))
                .fold(0, |bits, index| bits | (1 << index));
        } else if check || modifiers.command || modifiers.ctrl {
            mask ^= 1 << layer;
            if mask == 0 {
                mask = 1 << layer;
            }
            active = if mask
                & (1 << self.documents.active_session.document.active)
                != 0
            {
                self.documents.active_session.document.active
            } else {
                mask.trailing_zeros() as usize
            };
        } else {
            mask = 1 << layer;
        }
        self.select_layer(active);
        self.set_layer_selection(
            mask,
            if modifiers.shift { anchor } else { layer },
        );
    }
    pub(crate) fn prepare_layer_list(&mut self, ctx: &egui::Context) {
        self.documents.active_session.layer_ui.list.owned =
            self.documents.active_session.layer_ui.list.drag.is_some();
        let shown = self.workspace.panels.iter().any(|panel| {
            panel.palette == crate::workspace::Palette::Layers
                && panel.visible
                && !panel.collapsed
        });
        if self.documents.active_session.layer_ui.list.rename.is_some()
            && (!ctx.input(|input| input.focused)
                || self.dialog_open()
                || !shown
                || self.hide_panels
                || self
                    .documents
                    .active_session
                    .layer_ui
                    .list
                    .rename
                    .is_some_and(|id| {
                        self.documents
                            .active_session
                            .document
                            .layer_index(id)
                            .is_none()
                    }))
        {
            self.finish_layer_edit();
            self.documents.active_session.layer_ui.list.rename = None;
            self.documents.active_session.layer_ui.list.rename_focus = false;
        }
        if !ctx.input(|input| input.focused)
            || ctx.input(|input| input.key_pressed(egui::Key::Escape))
            || self.dialog_open()
            || !shown
            || self.hide_panels
            || self
                .documents
                .active_session
                .layer_ui
                .list
                .drag
                .as_ref()
                .is_some_and(|drag| {
                    drag.revision
                        != self.documents.active_session.document.revision
                })
        {
            self.documents.active_session.layer_ui.list.drag = None;
        }
    }
    pub(crate) fn begin_layer_rename(&mut self, layer: usize) {
        if layer >= self.documents.active_session.document.layers.len() {
            return;
        }
        self.finish_layer_edit();
        self.select_layer(layer);
        self.documents.active_session.layer_ui.list.rename =
            Some(self.documents.active_session.document.layers[layer].id);
        self.documents.active_session.layer_ui.list.rename_focus = true;
        self.documents.active_session.layer_ui.list.drag = None;
    }
    fn layer_name_editor(
        &mut self,
        ui: &mut egui::Ui,
        layer: usize,
        rect: Rect,
    ) {
        let mut name =
            self.documents.active_session.document.layers[layer].name.clone();
        let response = ui.put(
            rect,
            egui::TextEdit::singleline(&mut name)
                .id(ui.id().with((
                    "layer_name",
                    self.documents.active_session.document.layers[layer].id,
                )))
                .char_limit(128),
        );
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new("layer_name"), response.rect)
        });
        if self.documents.active_session.layer_ui.list.rename_focus {
            response.request_focus();
            if let Some(mut state) =
                egui::TextEdit::load_state(ui.ctx(), response.id)
            {
                state.cursor.set_char_range(Some(
                    egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(name.chars().count()),
                    ),
                ));
                state.store(ui.ctx(), response.id);
            }
            self.documents.active_session.layer_ui.list.rename_focus = false;
        }
        if response.changed()
            && name != self.documents.active_session.document.layers[layer].name
            && self.begin_layer_edit(LayerEditKind::Name, layer, false)
        {
            self.documents.active_session.document.layers[layer].name = name;
            self.documents.active_session.document.touch();
        }
        self.layer_edit_response(LayerEditKind::Name, &response);
        if response.lost_focus()
            || ui.input(|input| {
                input.key_pressed(egui::Key::Enter)
                    || input.key_pressed(egui::Key::Escape)
            })
        {
            self.finish_layer_edit();
            self.documents.active_session.layer_ui.list.rename = None;
        }
    }
    fn layer_eye(&mut self, ui: &mut egui::Ui, layer: usize, rect: Rect) {
        let response = ui.interact(
            rect,
            ui.id().with((
                "eye",
                self.documents.active_session.document.layers[layer].id,
            )),
            Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                self.documents.active_session.document.layers[layer].visible,
                "Layer visible",
            )
        });
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(
                egui::Id::new((
                    "layer_checkbox",
                    Command::LayerVisibility,
                    layer,
                )),
                rect,
            )
        });
        if self.documents.active_session.document.layers[layer].visible {
            paint_icon(ui, rect, Icon::Eye, Color32::from_gray(202));
        } else if response.hovered() {
            paint_icon(ui, rect, Icon::EyeSlash, Color32::from_gray(118));
        }
        if response.clicked() {
            if ui.input(|input| input.modifiers.alt) {
                self.solo_layer(layer);
            } else {
                self.documents.active_session.layer_ui.list.solo = None;
                self.execute_layer_command(Command::LayerVisibility, layer);
            }
        }
        response.on_hover_text("Show / hide layer · Alt-click: solo / restore");
    }
    fn solo_layer(&mut self, layer: usize) {
        let previous =
            self.documents.active_session.layer_ui.list.solo.take().filter(
                |flags| {
                    flags.len()
                        == self.documents.active_session.document.layers.len()
                        && flags.iter().all(|(id, _)| {
                            self.documents
                                .active_session
                                .document
                                .layer_index(*id)
                                .is_some()
                        })
                },
            );
        if let Some(flags) = previous {
            self.edit_named("Restore layer visibility", |document| {
                for target in &mut document.layers {
                    target.visible = flags
                        .iter()
                        .find(|(id, _)| *id == target.id)
                        .unwrap()
                        .1;
                }
            });
        } else {
            let flags = self
                .documents
                .active_session
                .document
                .layers
                .iter()
                .map(|target| (target.id, target.visible))
                .collect();
            self.edit_named("Solo layer", |document| {
                let mut visible =
                    document.layer_subtree_mask(layer).unwrap_or(1 << layer);
                let mut parent = document.layers[layer].parent;
                while let Some(index) =
                    parent.and_then(|id| document.layer_index(id))
                {
                    visible |= 1 << index;
                    parent = document.layers[index].parent;
                }
                for (index, target) in document.layers.iter_mut().enumerate() {
                    target.visible = visible & (1 << index) != 0;
                }
            });
            self.documents.active_session.layer_ui.list.solo = Some(flags);
        }
    }
    pub(crate) fn layer_rows(
        &mut self,
        ui: &mut egui::Ui,
        available_height: f32,
    ) {
        let key = self.documents.active_session.key;
        ui.push_id(key, |ui| self.layer_rows_contents(ui, available_height));
    }

    fn layer_rows_contents(
        &mut self,
        ui: &mut egui::Ui,
        available_height: f32,
    ) {
        let session = &mut *self.documents.active_session;
        session.layer_ui.list.collapsed.retain(|id| {
            session.document.layer_index(*id).is_some_and(|index| {
                session.document.layers[index].kind
                    == parfait_core::LayerKind::Folder
            })
        });
        self.documents.active_session.layer_ui.list.owned =
            self.documents.active_session.layer_ui.list.drag.is_some();
        let selected = self.selected_layer_mask();
        let anchor = self
            .documents
            .active_session
            .document
            .layer_index(self.documents.active_session.layer_ui.list.anchor)
            .unwrap_or(self.documents.active_session.document.active);
        self.set_layer_selection(selected, anchor);
        let mut targets = Vec::with_capacity(
            self.documents.active_session.document.layers.len(),
        );
        let scroll = egui::ScrollArea::vertical()
            .id_salt("layer_list")
            .max_height(available_height.max(42.))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.;
                for layer in
                    (0..self.documents.active_session.document.layers.len())
                        .rev()
                {
                    let (depth, hidden) = self.layer_depth_and_hidden(layer);
                    if hidden {
                        continue;
                    }
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 42.),
                        Sense::hover(),
                    );
                    let clip = rect.intersect(ui.clip_rect());
                    let chosen = self.selected_layer_mask() & (1 << layer) != 0;
                    ui.painter().rect_filled(
                        rect,
                        0.,
                        if chosen {
                            ui.visuals().selection.bg_fill
                        } else {
                            Color32::from_gray(60)
                        },
                    );
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        egui::Stroke::new(1., Color32::from_gray(40)),
                    );
                    let eye =
                        Rect::from_min_size(rect.min, egui::vec2(22., 42.));
                    self.layer_eye(ui, layer, eye);
                    let status = eye.translate(egui::vec2(22., 0.));
                    let check = ui.interact(
                        status,
                        ui.id().with((
                            "layer_check",
                            self.documents.active_session.document.layers
                                [layer]
                                .id,
                        )),
                        Sense::click(),
                    );
                    check.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            ui.is_enabled(),
                            chosen,
                            "Select additional layer",
                        )
                    });
                    if layer == self.documents.active_session.document.active {
                        paint_icon(
                            ui,
                            status,
                            Icon::Pen,
                            Color32::from_gray(220),
                        );
                    } else if chosen {
                        paint_icon(
                            ui,
                            status,
                            Icon::Check,
                            Color32::from_gray(220),
                        );
                    } else {
                        ui.painter().rect_stroke(
                            Rect::from_center_size(
                                status.center(),
                                egui::vec2(11., 11.),
                            ),
                            2.,
                            egui::Stroke::new(1., Color32::from_gray(43)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if check.clicked() {
                        self.layer_click(
                            layer,
                            ui.input(|input| input.modifiers),
                            true,
                        );
                        ui.ctx().request_repaint();
                    }
                    check.on_hover_text("Add / remove layer selection");
                    let has_mask =
                        self.documents.active_session.document.layers[layer]
                            .mask
                            .is_some();
                    let compact = rect.width() < 210.;
                    let thumbnail_size = if compact { 24. } else { 36. };
                    let controls_width = if compact { 22. } else { 44. };
                    let content_width = if has_mask {
                        thumbnail_size * 2. + 12.
                    } else {
                        thumbnail_size + 6.
                    };
                    let indent = (depth as f32 * 14.).min(
                        (rect.width()
                            - 47.
                            - content_width
                            - controls_width
                            - 36.)
                            .max(0.),
                    );
                    let thumbnail = Rect::from_min_size(
                        rect.min
                            + egui::vec2(
                                47. + indent,
                                (rect.height() - thumbnail_size) * 0.5,
                            ),
                        egui::Vec2::splat(thumbnail_size),
                    );
                    let folder = self.documents.active_session.document.layers
                        [layer]
                        .kind
                        == parfait_core::LayerKind::Folder;
                    if folder {
                        let arrow = Rect::from_center_size(
                            thumbnail.left_center() + egui::vec2(5., 0.),
                            egui::vec2(12., 20.),
                        );
                        let response = ui.interact(
                            arrow,
                            ui.id().with((
                                "folder_fold",
                                self.documents.active_session.document.layers
                                    [layer]
                                    .id,
                            )),
                            Sense::click(),
                        );
                        let closed = self
                            .documents
                            .active_session
                            .layer_ui
                            .list
                            .collapsed
                            .contains(
                                &self.documents.active_session.document.layers
                                    [layer]
                                    .id,
                            );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                !closed,
                                "폴더 펼치기 / 접기",
                            )
                        });
                        let center = arrow.center();
                        let points = if closed {
                            vec![
                                center + egui::vec2(-2., -4.),
                                center + egui::vec2(2., 0.),
                                center + egui::vec2(-2., 4.),
                            ]
                        } else {
                            vec![
                                center + egui::vec2(-4., -2.),
                                center + egui::vec2(0., 2.),
                                center + egui::vec2(4., -2.),
                            ]
                        };
                        ui.painter().add(egui::Shape::line(
                            points,
                            egui::Stroke::new(1.2, Color32::from_gray(200)),
                        ));
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new((
                                    "folder_fold",
                                    self.documents
                                        .active_session
                                        .document
                                        .layers[layer]
                                        .id,
                                )),
                                arrow,
                            )
                        });
                        if response.clicked() {
                            self.toggle_folder_view(
                                layer,
                                ui.input(|input| input.modifiers),
                            );
                            ui.ctx().request_repaint();
                        }
                        let icon = thumbnail
                            .translate(egui::vec2(6., 0.))
                            .shrink(if compact { 6. } else { 8. });
                        crate::widgets::paint_flat_icon(
                            ui,
                            icon,
                            Icon::Folder,
                            false,
                            if compact { 18. } else { 24. },
                        );
                        let folder_select = ui.interact(
                            icon,
                            ui.id().with((
                                "folder_select",
                                self.documents.active_session.document.layers
                                    [layer]
                                    .id,
                            )),
                            Sense::click(),
                        );
                        folder_select.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                self.documents.active_session.document.active
                                    == layer
                                    && !self.editing_layer_mask(),
                                "폴더 선택",
                            )
                        });
                        if folder_select.clicked() {
                            self.select_layer_target(layer, false);
                        }
                        folder_select.on_hover_text("폴더 선택");
                    } else if let Some(texture) =
                        self.render_cache.layer_previews.get(layer)
                    {
                        ui.painter().image(
                            texture.id(),
                            thumbnail,
                            Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().rect_filled(
                            thumbnail,
                            0.,
                            Color32::from_gray(115),
                        );
                    }
                    if !folder {
                        self.layer_target_thumbnail(
                            ui, layer, thumbnail, false,
                        );
                    }
                    if self.documents.active_session.document.layers[layer]
                        .draft
                    {
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                thumbnail.left_top() - egui::vec2(7., 0.),
                                thumbnail.left_bottom() - egui::vec2(5., 0.),
                            ),
                            0.,
                            Color32::from_rgb(122, 170, 226),
                        );
                    }
                    if self.documents.active_session.document.layers[layer]
                        .reference
                    {
                        let badge = Rect::from_min_size(
                            thumbnail.right_top() - egui::vec2(18., 0.),
                            egui::Vec2::splat(18.),
                        );
                        ui.painter().rect_filled(
                            badge,
                            1.,
                            Color32::from_black_alpha(200),
                        );
                        crate::widgets::paint_flat_icon(
                            ui,
                            badge,
                            Icon::Reference,
                            true,
                            18.,
                        );
                    }
                    if has_mask {
                        self.layer_target_thumbnail(
                            ui,
                            layer,
                            thumbnail
                                .translate(egui::vec2(thumbnail_size + 6., 0.)),
                            true,
                        );
                    }
                    let text_left = 47. + indent + content_width;
                    if self.documents.active_session.document.layers[layer]
                        .clipped
                    {
                        let indicator = Rect::from_min_max(
                            thumbnail.left_top() - egui::vec2(4., 0.),
                            thumbnail.left_bottom() - egui::vec2(2., 0.),
                        );
                        ui.painter().rect_filled(
                            indicator,
                            1.,
                            if self
                                .documents
                                .active_session
                                .document
                                .clipping_base(layer)
                                .is_some()
                            {
                                Color32::from_rgb(211, 135, 157)
                            } else {
                                Color32::from_gray(128)
                            },
                        );
                    }
                    let info = Rect::from_min_max(
                        rect.min + egui::vec2(text_left, 0.),
                        rect.max - egui::vec2(controls_width, 0.),
                    );
                    let response = ui.interact(
                        info,
                        ui.id().with((
                            "layer_row",
                            self.documents.active_session.document.layers
                                [layer]
                                .id,
                        )),
                        Sense::click_and_drag(),
                    );
                    let doc = &self.documents.active_session.document;
                    let row_label = doc.layers[layer].name.as_str();
                    if let Some([r, g, b]) =
                        doc.layer_color(doc.layers[layer].id)
                    {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                rect.min,
                                egui::vec2(3., rect.height()),
                            ),
                            0.,
                            Color32::from_rgb(r, g, b),
                        );
                    }
                    if let Some(node) = doc.native_content(layer) {
                        let icon = match node.body().as_ref() {
                            parfait_core::LayerContent::Text(_) => Icon::Text,
                            parfait_core::LayerContent::Vector(_) => {
                                Icon::Vector
                            }
                        };
                        let badge = Rect::from_min_size(
                            thumbnail.right_bottom() - egui::vec2(13., 13.),
                            egui::vec2(13., 13.),
                        );
                        ui.painter().rect_filled(
                            badge,
                            1.,
                            Color32::from_gray(44),
                        );
                        paint_icon(ui, badge, icon, Color32::from_gray(225));
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            ui.is_enabled(),
                            chosen,
                            row_label,
                        )
                    });
                    let name = Rect::from_min_max(
                        rect.min + egui::vec2(text_left, 20.),
                        rect.max - egui::vec2(controls_width, 1.),
                    );
                    let text = format!(
                        "{:.0}%  {}{}{}",
                        self.documents.active_session.document.layers[layer]
                            .opacity
                            * 100.,
                        crate::layer_controls::blend_label(
                            self.documents.active_session.document.layers
                                [layer]
                                .blend_mode
                        ),
                        if self.documents.active_session.document.layers[layer]
                            .reference
                        {
                            "  참조"
                        } else {
                            ""
                        },
                        if self.documents.active_session.document.layers[layer]
                            .draft
                        {
                            "  밑그림"
                        } else {
                            ""
                        },
                    );
                    clipped_text(
                        ui,
                        rect.min + egui::vec2(text_left, 4.),
                        &text,
                        11.,
                        name.right() - name.left(),
                        if self.documents.active_session.document.layers[layer]
                            .blend_mode
                            == parfait_core::BlendMode::Through
                            && self.documents.active_session.document.layers
                                [layer]
                                .clipped
                        {
                            crate::layer_controls::SUPPRESSED_BLEND
                        } else {
                            ui.visuals().text_color()
                        },
                    );
                    if self.documents.active_session.layer_ui.list.rename
                        == Some(
                            self.documents.active_session.document.layers
                                [layer]
                                .id,
                        )
                    {
                        self.layer_name_editor(ui, layer, name);
                    } else {
                        clipped_text(
                            ui,
                            name.min,
                            row_label,
                            12.,
                            name.width(),
                            ui.visuals().text_color(),
                        );
                    }
                    let lock = Rect::from_min_size(
                        egui::pos2(rect.right() - 21., rect.top() + 20.),
                        egui::vec2(20., 21.),
                    );
                    let locked = ui.interact(
                        lock,
                        ui.id().with((
                            "layer_lock",
                            self.documents.active_session.document.layers
                                [layer]
                                .id,
                        )),
                        Sense::click(),
                    );
                    locked.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            ui.is_enabled(),
                            self.documents.active_session.document.layers
                                [layer]
                                .locked,
                            "Layer locked",
                        )
                    });
                    if self.documents.active_session.document.layers[layer]
                        .locked
                    {
                        paint_icon(
                            ui,
                            lock,
                            Icon::Lock,
                            Color32::from_gray(195),
                        );
                    } else if locked.hovered() {
                        paint_icon(
                            ui,
                            lock,
                            Icon::Unlock,
                            Color32::from_gray(140),
                        );
                    }
                    if locked.clicked() {
                        self.execute_layer_command(Command::LayerLock, layer);
                        ui.ctx().request_repaint();
                    }
                    locked
                        .on_hover_text(self.keymap.tooltip(Command::LayerLock));
                    let alpha_rect = if compact {
                        Rect::from_min_size(lock.min, egui::Vec2::ZERO)
                    } else {
                        lock.translate(egui::vec2(-22., 0.))
                    };
                    let alpha_enabled = !compact
                        && self.documents.active_session.document.layers[layer]
                            .kind
                            == parfait_core::LayerKind::Raster;
                    let alpha = ui.interact(
                        alpha_rect,
                        ui.id().with((
                            "layer_alpha_lock",
                            self.documents.active_session.document.layers
                                [layer]
                                .id,
                        )),
                        if alpha_enabled {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    alpha.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            ui.is_enabled() && alpha_enabled,
                            self.documents.active_session.document.layers
                                [layer]
                                .alpha_locked,
                            "Lock transparent pixels",
                        )
                    });
                    if alpha_enabled
                        && (self.documents.active_session.document.layers
                            [layer]
                            .alpha_locked
                            || alpha.hovered())
                    {
                        widgets_alpha_icon(
                            ui,
                            alpha_rect,
                            self.documents.active_session.document.layers
                                [layer]
                                .alpha_locked,
                        );
                    }
                    if alpha.clicked() {
                        self.execute_layer_command(
                            Command::LayerAlphaLock,
                            layer,
                        );
                        ui.ctx().request_repaint();
                    }
                    alpha.on_hover_text(
                        self.keymap.tooltip(Command::LayerAlphaLock),
                    );
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new((
                                "layer_checkbox",
                                Command::LayerLock,
                                layer,
                            )),
                            lock,
                        );
                        data.insert_temp(
                            egui::Id::new(("layer_row", layer)),
                            info,
                        );
                        data.insert_temp(
                            egui::Id::new(("layer_check", layer)),
                            status,
                        );
                        data.insert_temp(
                            egui::Id::new(("layer_label", layer)),
                            name,
                        );
                        data.insert_temp(
                            egui::Id::new((
                                "layer_checkbox",
                                Command::LayerAlphaLock,
                                layer,
                            )),
                            alpha_rect,
                        );
                    });
                    if response.double_clicked()
                        && self
                            .documents
                            .active_session
                            .layer_ui
                            .list
                            .rename
                            .is_none()
                    {
                        self.begin_layer_rename(layer);
                        ui.ctx().request_repaint();
                    }
                    response.context_menu(|ui| {
                        if self.selected_layer_mask() & (1 << layer) == 0 {
                            self.layer_click(layer, Default::default(), false);
                        }
                        if ui.button("Rename layer").clicked() {
                            self.begin_layer_rename(layer);
                            ui.close();
                        }
                        ui.separator();
                        for command in [
                            Command::NewTextLayer,
                            Command::NewVectorLayer,
                            Command::EditNativeLayer,
                            Command::RasterizeNativeLayer,
                            Command::NewLayerFolder,
                            Command::GroupLayersInFolder,
                            Command::UngroupLayerFolder,
                            Command::DuplicateLayer,
                            Command::DeleteLayer,
                            Command::MoveLayerUp,
                            Command::MoveLayerDown,
                        ] {
                            self.command_menu_item(ui, command);
                        }
                        ui.separator();
                        self.layer_operations_menu(ui);
                        ui.menu_button("Layer mask", |ui| {
                            self.layer_mask_menu(ui)
                        });
                        ui.separator();
                        self.command_toggle(
                            ui,
                            Command::LayerVisibility,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .visible,
                        );
                        self.command_toggle(
                            ui,
                            Command::LayerLock,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .locked,
                        );
                        self.command_toggle(
                            ui,
                            Command::LayerAlphaLock,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .alpha_locked,
                        );
                        self.command_toggle(
                            ui,
                            Command::LayerClipping,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .clipped,
                        );
                        self.command_toggle(
                            ui,
                            Command::LayerReference,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .reference,
                        );
                        self.command_toggle(
                            ui,
                            Command::LayerDraft,
                            self.documents.active_session.document.layers
                                [self.documents.active_session.document.active]
                                .draft,
                        );
                        self.command_menu_item(ui, Command::LayerDetails);
                    });
                    if clip.is_positive() {
                        targets.push((layer, info.intersect(clip), rect));
                    }
                }
            });
        let viewport = scroll.inner_rect;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new("layer_list_viewport"), viewport)
        });
        self.layer_drag_events(ui, &targets, viewport);
        if let Some(drag) = &self.documents.active_session.layer_ui.list.drag
            && drag.moved
            && let Some(pointer) =
                ui.input(|input| input.pointer.interact_pos())
        {
            if let Some(target) =
                self.layer_drop_target(&targets, viewport, pointer)
            {
                let reason = self.layer_drop_reason(drag.mask, target);
                let color = if reason.is_none() {
                    crate::theme::ACCENT
                } else {
                    Color32::RED
                };
                let painter = ui.painter().with_clip_rect(viewport);
                match target {
                    LayerDrop::Move { y, .. } => {
                        painter.hline(
                            viewport.x_range(),
                            y.clamp(viewport.top(), viewport.bottom()),
                            egui::Stroke::new(2., color),
                        );
                    }
                    LayerDrop::Inside { rect, .. }
                    | LayerDrop::Group { rect, .. } => {
                        let rect = rect.intersect(viewport).shrink(1.);
                        painter.rect_filled(
                            rect,
                            0.,
                            color.gamma_multiply(0.12),
                        );
                        painter.rect_stroke(
                            rect,
                            0.,
                            egui::Stroke::new(1.5, color),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                let text = reason.unwrap_or(match target {
                    LayerDrop::Move { .. } => "순서 변경",
                    LayerDrop::Inside { .. } => "폴더 안으로 이동",
                    LayerDrop::Group { .. } => "새 폴더로 묶기",
                });
                egui::Area::new(egui::Id::new("layer_drop_hint"))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .fixed_pos(pointer + egui::vec2(14., 18.))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.colored_label(color, text);
                        });
                    });
            }
            let direction = if pointer.y < viewport.top() + 18. {
                -1.
            } else if pointer.y > viewport.bottom() - 18. {
                1.
            } else {
                0.
            };
            if direction != 0. && viewport.x_range().contains(pointer.x) {
                let mut state = scroll.state;
                let previous_offset = state.offset.y;
                state.offset.y = (state.offset.y
                    + direction
                        * 300.
                        * ui.input(|input| input.stable_dt).min(0.05))
                .clamp(0., (scroll.content_size.y - viewport.height()).max(0.));
                state.store(ui.ctx(), scroll.id);
                if state.offset.y != previous_offset {
                    ui.ctx().request_repaint();
                }
            }
        }
    }
    fn layer_drop_target(
        &self,
        targets: &[(usize, Rect, Rect)],
        viewport: Rect,
        pointer: Pos2,
    ) -> Option<LayerDrop> {
        if !viewport.contains(pointer) {
            return None;
        }
        let document = &self.documents.active_session.document;
        for (index, rect, _) in targets {
            if pointer.y <= rect.bottom() {
                let layer = &document.layers[*index];
                if pointer.y > rect.top() + 10.
                    && pointer.y < rect.bottom() - 10.
                {
                    return Some(
                        if layer.kind == parfait_core::LayerKind::Folder {
                            LayerDrop::Inside { folder: *index, rect: *rect }
                        } else {
                            LayerDrop::Group { target: *index, rect: *rect }
                        },
                    );
                }
                if pointer.y < rect.center().y {
                    return Some(LayerDrop::Move {
                        boundary: index + 1,
                        parent: layer.parent,
                        y: rect.top(),
                    });
                }
                return Some(LayerDrop::Move {
                    boundary: document
                        .layer_subtree_mask(*index)?
                        .trailing_zeros()
                        as usize,
                    parent: layer.parent,
                    y: rect.bottom(),
                });
            }
        }
        let (index, rect, _) = targets.last()?;
        Some(LayerDrop::Move {
            boundary: document.layer_subtree_mask(*index)?.trailing_zeros()
                as usize,
            parent: document.layers[*index].parent,
            y: rect.bottom(),
        })
    }

    fn layer_drop_reason(
        &self,
        selection: u64,
        target: LayerDrop,
    ) -> Option<&'static str> {
        let document = &self.documents.active_session.document;
        match target {
            LayerDrop::Group { target, .. } => {
                self.group_layer_drop_reason(selection, target)
            }
            LayerDrop::Inside { folder, .. } => {
                parfait_storage::layer_reparent_error(
                    document,
                    selection,
                    folder,
                    Some(document.layers[folder].id),
                )
            }
            // Flat raster documents keep their existing reorder semantics.
            LayerDrop::Move { boundary, parent, .. }
                if document.has_non_raster_layers() =>
            {
                parfait_storage::layer_reparent_error(
                    document, selection, boundary, parent,
                )
            }
            LayerDrop::Move { .. } => None,
        }
    }

    fn layer_drag_events(
        &mut self,
        ui: &egui::Ui,
        targets: &[(usize, Rect, Rect)],
        viewport: Rect,
    ) {
        let events = ui.input(|input| input.events.clone());
        let selection_before = self.selected_layer_mask();
        let revision_before = self.documents.active_session.document.revision;
        if self
            .documents
            .active_session
            .layer_ui
            .list
            .drag
            .as_ref()
            .is_some_and(|drag| {
                drag.revision != self.documents.active_session.document.revision
            })
            || ui.input(|input| {
                !input.focused || input.key_pressed(egui::Key::Escape)
            })
            || !ui.is_enabled()
        {
            self.documents.active_session.layer_ui.list.drag = None;
            return;
        }
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if self
                    .documents
                    .active_session
                    .layer_ui
                    .list
                    .rename
                    .is_none()
                    && self
                        .documents
                        .active_session
                        .editing
                        .stroke
                        .is_none()
                    && !self.dialog_open()
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    if let Some((layer, _, _)) =
                        targets.iter().find(|(_, info, _)| info.contains(pos))
                    {
                        let selected =
                            self.selected_layer_mask() & (1 << layer) != 0;
                        let collapse = (selected
                            && !modifiers.shift
                            && !modifiers.command
                            && !modifiers.ctrl)
                            .then_some(*layer);
                        if collapse.is_none() {
                            self.layer_click(*layer, modifiers, false);
                        }
                        self.documents.active_session.layer_ui.list.drag =
                            Some(LayerDrag {
                                mask: self.selected_layer_mask(),
                                revision: self
                                    .documents
                                    .active_session
                                    .document
                                    .revision,
                                origin: pos,
                                moved: false,
                                collapse,
                            });
                        self.documents.active_session.layer_ui.list.owned =
                            true;
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    if let Some(drag) =
                        &mut self.documents.active_session.layer_ui.list.drag
                    {
                        drag.moved |= drag.origin.distance(pos) > 4.;
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    if let Some(mut drag) =
                        self.documents.active_session.layer_ui.list.drag.take()
                    {
                        drag.moved |= drag.origin.distance(pos) > 4.;
                        if drag.moved {
                            if let Some(target) =
                                self.layer_drop_target(targets, viewport, pos)
                            {
                                if let Some(reason) =
                                    self.layer_drop_reason(drag.mask, target)
                                {
                                    self.notice = Some(reason.into());
                                } else {
                                    match target {
                                        LayerDrop::Group { target, .. } => self
                                            .group_layer_drop(
                                                drag.mask, target,
                                            ),
                                        LayerDrop::Inside {
                                            folder, ..
                                        } => self.move_tree_selection(
                                            drag.mask,
                                            folder,
                                            Some(
                                                self.documents
                                                    .active_session
                                                    .document
                                                    .layers[folder]
                                                    .id,
                                            ),
                                        ),
                                        LayerDrop::Move {
                                            boundary,
                                            parent,
                                            ..
                                        } => {
                                            if self
                                                .documents
                                                .active_session
                                                .document
                                                .has_non_raster_layers()
                                            {
                                                self.move_tree_selection(
                                                    drag.mask, boundary, parent,
                                                );
                                            } else {
                                                self.reorder_layer_mask(
                                                    drag.mask, boundary,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        } else if let Some(layer) = drag.collapse {
                            self.layer_click(layer, Default::default(), false);
                        }
                    }
                }
                egui::Event::PointerGone
                | egui::Event::WindowFocused(false) => {
                    self.documents.active_session.layer_ui.list.drag = None;
                }
                _ => {}
            }
        }
        if selection_before != self.selected_layer_mask()
            || revision_before
                != self.documents.active_session.document.revision
        {
            ui.ctx().request_repaint();
        }
    }
    pub(crate) fn reorder_layer_mask(&mut self, mask: u64, boundary: usize) {
        if self.documents.active_session.document.has_non_raster_layers() {
            self.move_tree_selection(
                mask,
                boundary,
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .parent,
            );
            return;
        }
        let count = self.documents.active_session.document.layers.len();
        if boundary > count
            || mask == 0
            || self.documents.active_session.editing.stroke.is_some()
            || self.dialog_open()
        {
            return;
        }
        let mut order: Vec<_> =
            (0..count).filter(|index| mask & (1 << index) == 0).collect();
        let insertion =
            (0..boundary).filter(|index| mask & (1 << index) == 0).count();
        order.splice(
            insertion..insertion,
            (0..count).filter(|index| mask & (1 << index) != 0),
        );
        self.apply_layer_order(order, mask);
    }
    pub(crate) fn apply_layer_order(&mut self, order: Vec<usize>, mask: u64) {
        self.apply_layer_order_named(order, mask, "Reorder layers");
    }
    pub(crate) fn apply_layer_order_named(
        &mut self,
        order: Vec<usize>,
        mask: u64,
        label: &str,
    ) {
        if order.iter().copied().eq(0..self
            .documents
            .active_session
            .document
            .layers
            .len())
        {
            return;
        }
        let active = order
            .iter()
            .position(|index| {
                *index == self.documents.active_session.document.active
            })
            .unwrap();
        let selected = order
            .iter()
            .enumerate()
            .filter(|(_, index)| mask & (1 << **index) != 0)
            .fold(0, |bits, (index, _)| bits | (1 << index));
        self.edit_named(label, |document| {
            let mut layers: Vec<_> = std::mem::take(&mut document.layers)
                .into_iter()
                .map(Some)
                .collect();
            document.layers = order
                .iter()
                .map(|index| layers[*index].take().unwrap())
                .collect();
            document.active = active;
            document.touch();
        });
        self.set_layer_selection(selected, active);
        self.documents.active_session.layer_ui.list.solo = None;
    }
}

fn widgets_alpha_icon(ui: &egui::Ui, rect: Rect, selected: bool) {
    crate::widgets::paint_flat_icon(ui, rect, Icon::AlphaLock, selected, 20.);
}

fn paint_icon(ui: &egui::Ui, rect: Rect, icon: Icon, color: Color32) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.glyph(),
        egui::FontId::new(13., egui::FontFamily::Name("icons".into())),
        color,
    );
}
fn clipped_text(
    ui: &egui::Ui,
    origin: Pos2,
    text: &str,
    size: f32,
    width: f32,
    color: Color32,
) {
    let galley = ui.painter().layout(
        text.to_owned(),
        egui::FontId::proportional(size),
        color,
        f32::INFINITY,
    );
    ui.painter()
        .with_clip_rect(ui.clip_rect().intersect(Rect::from_min_size(
            origin,
            egui::vec2(width.max(0.), size + 5.),
        )))
        .galley(origin, galley, color);
}
