use crate::{app::Parfait, shortcuts::Command};
use parfait_core::{Align, ContentBox, LayerId, PixelRect};
use std::cell::RefCell;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum AlignReference {
    Bounds,
    Canvas,
    Selection,
    Guides,
    #[default]
    Auto,
}
type CachedBounds = Result<Vec<(LayerId, ContentBox)>, &'static str>;
struct AlignmentCache {
    revision: u64,
    selected: u64,
    flags: (bool, bool),
    bounds: CachedBounds,
    selection: Option<PixelRect>,
}
#[derive(Default)]
pub(crate) struct AlignSettings {
    pub reference: AlignReference,
    pub text_ink: bool,
    pub vector_center: bool,
    pub guides: Option<PixelRect>,
    cache: RefCell<Option<AlignmentCache>>,
}
impl Command {
    pub(crate) fn alignment(self) -> Option<Align> {
        use Command::*;
        Some(match self {
            EditAlignTop => Align::Top,
            EditAlignCenterY => Align::CenterY,
            EditAlignBottom => Align::Bottom,
            EditAlignLeft => Align::Left,
            EditAlignCenterX => Align::CenterX,
            EditAlignRight => Align::Right,
            EditDistributeTop => Align::DistributeTop,
            EditDistributeCenterY => Align::DistributeCenterY,
            EditDistributeBottom => Align::DistributeBottom,
            EditDistributeLeft => Align::DistributeLeft,
            EditDistributeCenterX => Align::DistributeCenterX,
            EditDistributeRight => Align::DistributeRight,
            EditSpaceY => Align::SpaceY,
            EditSpaceX => Align::SpaceX,
            _ => return None,
        })
    }
    pub(crate) fn align_reference(self) -> Option<AlignReference> {
        Some(match self {
            Self::EditRefBounds => AlignReference::Bounds,
            Self::EditRefCanvas => AlignReference::Canvas,
            Self::EditRefSelection => AlignReference::Selection,
            Self::EditRefGuides => AlignReference::Guides,
            Self::EditRefAuto => AlignReference::Auto,
            _ => return None,
        })
    }
}
impl Parfait {
    pub(crate) fn alignment_plan(
        &self,
        command: Command,
    ) -> Result<Vec<(LayerId, [i32; 2])>, &'static str> {
        let session = &self.documents.active_session;
        let doc = &session.document;
        let selection = self.selected_layer_mask();
        let settings = &session.editing.align;
        let mut cache = settings.cache.borrow_mut();
        if cache.as_ref().is_none_or(|cached| {
            cached.revision != doc.revision
                || cached.selected != selection
                || cached.flags != (settings.text_ink, settings.vector_center)
        }) {
            *cache = Some(AlignmentCache {
                revision: doc.revision,
                selected: selection,
                flags: (settings.text_ink, settings.vector_center),
                bounds: doc.content_alignment_bounds(
                    selection,
                    settings.text_ink,
                    settings.vector_center,
                ),
                selection: doc.selection_bounds(),
            });
        }
        let cached = cache.as_ref().unwrap();
        let canvas = PixelRect { x0: 0, y0: 0, x1: doc.width, y1: doc.height };
        let reference = match settings.reference {
            AlignReference::Bounds => None,
            AlignReference::Canvas => Some(canvas),
            AlignReference::Selection => Some(
                cached
                    .selection
                    .ok_or("정렬 기준으로 쓸 선택 범위가 없습니다.")?,
            ),
            AlignReference::Guides => Some(
                settings.guides.ok_or("가이드 기준을 먼저 설정해 주세요.")?,
            ),
            AlignReference::Auto => cached
                .selection
                .or_else(|| (selection.count_ones() == 1).then_some(canvas)),
        };
        match &cached.bounds {
            Ok(bounds) => doc.plan_content_alignment(
                bounds,
                command.alignment().unwrap(),
                reference,
            ),
            Err(reason) => Err(*reason),
        }
    }
    pub(crate) fn execute_alignment(&mut self, command: Command) {
        match self.alignment_plan(command) {
            Ok(plan) => {
                let bytes = self
                    .documents
                    .active_session
                    .document
                    .logical_pixel_bytes();
                if !self.reserve_pixel_headroom(bytes.saturating_mul(2)) {
                    self.notice = Some(
                        "정렬 결과와 실행 취소를 위한 메모리가 부족합니다."
                            .into(),
                    );
                    return;
                }
                self.edit_named(command.label(), |doc| {
                    doc.translate_layer_pixels(&plan);
                });
            }
            Err(reason) => self.notice = Some(reason.into()),
        }
    }
    pub(crate) fn alignment_guides(&self, ui: &egui::Ui, rect: egui::Rect) {
        let session = &self.documents.active_session;
        if session.editing.align.reference != AlignReference::Guides {
            return;
        }
        let Some(bounds) = session.editing.align.guides else {
            return;
        };
        let size = egui::vec2(
            session.document.width as f32,
            session.document.height as f32,
        );
        let points = [
            egui::vec2(bounds.x0 as f32, bounds.y0 as f32),
            egui::vec2(bounds.x1 as f32, bounds.y0 as f32),
            egui::vec2(bounds.x1 as f32, bounds.y1 as f32),
            egui::vec2(bounds.x0 as f32, bounds.y1 as f32),
        ]
        .map(|p| session.view.screen(p, rect, size));
        let painter = ui.painter().with_clip_rect(rect);
        for i in 0..4 {
            painter.line_segment(
                [points[i], points[(i + 1) % 4]],
                egui::Stroke::new(1., egui::Color32::from_rgb(100, 185, 190)),
            );
        }
    }
}
use eframe::egui;
