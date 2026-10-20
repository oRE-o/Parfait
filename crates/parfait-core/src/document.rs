use crate::{
    BlendMode,
    blend::{composite_blend, composite_opaque},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Half-open pixel bounds. Transient renderer state, never serialized into projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}
impl PixelRect {
    pub fn union(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
    pub fn clipped(self, width: u32, height: u32) -> Self {
        let x0 = self.x0.min(width);
        let y0 = self.y0.min(height);
        Self {
            x0,
            y0,
            x1: self.x1.min(width).max(x0),
            y1: self.y1.min(height).max(y0),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub alpha_locked: bool,
    pub clipped: bool,
    pub opacity: f32,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(deserialize_with = "crate::deserialize_pixel_buffer")]
    pub pixels: Arc<Vec<u8>>,
    #[serde(default)]
    pub mask: Option<crate::LayerMask>,
    #[serde(default)]
    pub reference: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub kind: crate::LayerKind,
    #[serde(default)]
    pub parent: Option<LayerId>,
}

#[derive(
    Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, Debug,
)]
#[serde(transparent)]
pub struct LayerId(pub u64);

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub name: String,
    /// Bottom to top. Selection is an index into this list.
    pub layers: Vec<Layer>,
    pub active: usize,
    pub revision: u64,
    pub next_layer_id: u64,
    pub selection: Option<crate::MaskCoverage>,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Self::new_background(width, height, false)
    }

    pub(crate) fn new_background(width: u32, height: u32, paper: bool) -> Self {
        assert!(width > 0 && height > 0 && width <= 4096 && height <= 4096);
        let len = width as usize * height as usize * 4;
        Self {
            width,
            height,
            name: "Untitled".into(),
            selection: None,
            layers: vec![
                Layer {
                    id: LayerId(1),
                    name: "Paper".into(),
                    visible: true,
                    locked: !paper,
                    alpha_locked: false,
                    clipped: false,
                    opacity: 1.,
                    blend_mode: BlendMode::Normal,
                    pixels: Arc::new(if paper {
                        Vec::new()
                    } else {
                        vec![255; len]
                    }),
                    mask: None,
                    reference: false,
                    draft: false,
                    kind: if paper {
                        crate::LayerKind::Paper([255; 3])
                    } else {
                        crate::LayerKind::Raster
                    },
                    parent: None,
                },
                Layer {
                    id: LayerId(2),
                    name: "Layer 1".into(),
                    visible: true,
                    locked: false,
                    alpha_locked: false,
                    clipped: false,
                    opacity: 1.,
                    blend_mode: BlendMode::Normal,
                    pixels: Arc::new(vec![0; len]),
                    mask: None,
                    reference: false,
                    draft: false,
                    kind: crate::LayerKind::Raster,
                    parent: None,
                },
            ],
            active: 1,
            revision: 1,
            next_layer_id: 3,
        }
    }
    pub fn validate(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 4096
            && self.height <= 4096
            && !self.layers.is_empty()
            && self.layers.len() <= 64
            && self.active < self.layers.len()
            && self.folder_layout().is_some()
            && self.selection.as_ref().is_none_or(|coverage| match coverage {
                crate::MaskCoverage::Uniform(_) => true,
                crate::MaskCoverage::Dense(values) => {
                    values.len() == self.width as usize * self.height as usize
                }
            })
            && self.layers.iter().enumerate().all(|(index, layer)| {
                layer.id.0 > 0
                    && layer.id.0 < self.next_layer_id
                    && self.layers[..index]
                        .iter()
                        .all(|previous| previous.id != layer.id)
            })
            && self.logical_pixel_bytes() <= 512 * 1024 * 1024
            && self.layers.iter().enumerate().all(|(index, l)| {
                (match l.kind {
                    crate::LayerKind::Raster => {
                        l.blend_mode != BlendMode::Through
                            && l.pixels.len()
                                == self.width as usize
                                    * self.height as usize
                                    * 4
                    }
                    crate::LayerKind::Folder => {
                        l.pixels.is_empty() && !l.alpha_locked
                    }
                    crate::LayerKind::Fill(_) => {
                        l.pixels.is_empty()
                            && !l.alpha_locked
                            && l.blend_mode != BlendMode::Through
                    }
                    crate::LayerKind::Paper(_) => {
                        index == 0
                            && l.parent.is_none()
                            && l.pixels.is_empty()
                            && !l.alpha_locked
                            && !l.clipped
                            && l.opacity == 1.
                            && l.blend_mode == BlendMode::Normal
                            && l.mask.is_none()
                            && !l.reference
                            && !l.draft
                    }
                }) && l.mask.as_ref().is_none_or(|mask| match &mask.coverage {
                    crate::MaskCoverage::Uniform(_) => true,
                    crate::MaskCoverage::Dense(values) => {
                        values.len()
                            == self.width as usize * self.height as usize
                    }
                }) && l.opacity.is_finite()
                    && (0.0..=1.0).contains(&l.opacity)
            })
    }
    pub fn touch(&mut self) {
        self.revision += 1;
    }
    pub fn same_edit_state(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && match (&self.selection, &other.selection) {
                (None, None) => true,
                (Some(current), Some(previous)) => {
                    current.same_edit_state(previous)
                }
                _ => false,
            }
            && self.layers.len() == other.layers.len()
            && self.layers.iter().zip(&other.layers).all(
                |(current, previous)| {
                    current.id == previous.id
                        && current.kind == previous.kind
                        && current.parent == previous.parent
                        && current.reference == previous.reference
                        && current.draft == previous.draft
                        && current.alpha_locked == previous.alpha_locked
                        && current.clipped == previous.clipped
                        && current.name == previous.name
                        && current.visible == previous.visible
                        && current.locked == previous.locked
                        && current.opacity == previous.opacity
                        && current.blend_mode == previous.blend_mode
                        && match (&current.mask, &previous.mask) {
                            (None, None) => true,
                            (Some(current), Some(previous)) => {
                                current.enabled == previous.enabled
                                    && current
                                        .coverage
                                        .same_edit_state(&previous.coverage)
                            }
                            _ => false,
                        }
                        && Arc::ptr_eq(&current.pixels, &previous.pixels)
                },
            )
    }
    pub fn layer_at(&self, column: u32, row: u32) -> Option<usize> {
        if column >= self.width || row >= self.height {
            return None;
        }
        if self.has_non_raster_layers() {
            return self.folder_layer_at(column, row);
        }
        let alpha = ((row * self.width + column) * 4 + 3) as usize;
        self.layers.iter().enumerate().rev().find_map(|(index, layer)| {
            let base = self.clipping_base(index)?;
            let base = &self.layers[base];
            (layer.visible
                && layer.opacity > 0.
                && layer.effective_alpha(alpha / 4) > 0
                && base.visible
                && base.opacity > 0.
                && base.effective_alpha(alpha / 4) > 0)
                .then_some(index)
        })
    }
    pub fn clipping_base(&self, index: usize) -> Option<usize> {
        if !self.layers.get(index)?.clipped {
            Some(index)
        } else {
            self.layer_clipping_base_below(index, 0)
        }
    }
    pub fn layer_clipping_base_below(
        &self,
        index: usize,
        excluded: u64,
    ) -> Option<usize> {
        let parent = self.layers.get(index)?.parent;
        self.layers[..index]
            .iter()
            .enumerate()
            .rfind(|(below, layer)| {
                layer.parent == parent
                    && !layer.clipped
                    && excluded & (1 << below) == 0
            })
            .map(|(below, _)| below)
            .filter(|below| {
                self.layers[*below].blend_mode != BlendMode::Through
            })
    }
    pub fn add_layer(&mut self) {
        let Some(active) = self.layers.get(self.active) else {
            return;
        };
        let parent = if active.kind == crate::LayerKind::Folder {
            Some(active.id)
        } else {
            active.parent
        };
        if self.folder_edit_reason(parent).is_some() {
            return;
        }
        if self.layers.len() >= 64
            || self.logical_pixel_bytes().saturating_add(
                u64::from(self.width)
                    .saturating_mul(u64::from(self.height))
                    .saturating_mul(4),
            ) > 512 * 1024 * 1024
        {
            return;
        }
        let Some(id) = self.allocate_layer_id() else {
            return;
        };
        let layer = Layer {
            id,
            name: format!("Layer {}", self.layers.len()),
            visible: true,
            locked: false,
            alpha_locked: false,
            clipped: self.layers.get(self.active + 1).is_some_and(|layer| {
                self.layers[self.active].kind == crate::LayerKind::Raster
                    && layer.parent == self.layers[self.active].parent
                    && layer.clipped
            }),
            opacity: 1.,
            blend_mode: BlendMode::Normal,
            pixels: Arc::new(vec![
                0;
                self.width as usize
                    * self.height as usize
                    * 4
            ]),
            mask: None,
            reference: false,
            draft: false,
            kind: crate::LayerKind::Raster,
            parent,
        };
        if self.layers[self.active].kind != crate::LayerKind::Folder {
            self.active += 1;
        }
        self.layers.insert(self.active, layer);
        self.touch();
    }
    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|layer| layer.id == id)
    }
    pub fn can_allocate_layers(&self, count: usize) -> bool {
        self.next_layer_id > 0
            && self.next_layer_id.checked_add(count as u64).is_some()
    }
    pub fn allocate_layer_id(&mut self) -> Option<LayerId> {
        let next = self.next_layer_id.checked_add(1)?;
        if self.next_layer_id == 0 {
            return None;
        }
        let id = LayerId(self.next_layer_id);
        self.next_layer_id = next;
        Some(id)
    }
    pub fn composite(&self) -> Vec<u8> {
        self.composite_region(PixelRect {
            x0: 0,
            y0: 0,
            x1: self.width,
            y1: self.height,
        })
    }
    /// Composite a clipped, tightly packed region without allocating the full canvas.
    pub fn composite_region(&self, rect: PixelRect) -> Vec<u8> {
        self.composite_region_layers(rect, u64::MAX)
    }
    pub(crate) fn composite_region_layers(
        &self,
        rect: PixelRect,
        mask: u64,
    ) -> Vec<u8> {
        if self.has_non_raster_layers() {
            return self.composite_folder_region(rect, mask);
        }
        let r = rect.clipped(self.width, self.height);
        let stride = (r.x1 - r.x0) as usize * 4;
        let mut out = vec![0; stride * (r.y1 - r.y0) as usize];
        if stride == 0 {
            return out;
        }
        let mut clipped_row = Vec::<[u8; 4]>::new();
        for (index, layer) in
            self.layers.iter().enumerate().filter(|(index, layer)| {
                mask & (1 << index) != 0
                    && !layer.clipped
                    && layer.visible
                    && layer.opacity > 0.
            })
        {
            let end = self.layers[index + 1..]
                .iter()
                .position(|layer| !layer.clipped)
                .map_or(self.layers.len(), |offset| index + 1 + offset);
            let mut clips = [layer; 64];
            let mut clip_count = 0;
            for (offset, clip) in self.layers[index + 1..end].iter().enumerate()
            {
                if mask & (1 << (index + 1 + offset)) != 0
                    && clip.visible
                    && clip.opacity > 0.
                {
                    clips[clip_count] = clip;
                    clip_count += 1;
                }
            }
            if clip_count == 0 {
                for (row_index, row) in out.chunks_exact_mut(stride).enumerate()
                {
                    let start = ((r.y0 as usize + row_index)
                        * self.width as usize
                        + r.x0 as usize)
                        * 4;
                    let source_row =
                        layer.pixels[start..start + stride].as_chunks::<4>().0;
                    match layer
                        .mask
                        .as_ref()
                        .filter(|mask| mask.enabled)
                        .map(|mask| &mask.coverage)
                    {
                        None | Some(crate::MaskCoverage::Uniform(255)) => {
                            composite_flat_row(
                                row,
                                source_row,
                                layer.opacity,
                                layer.blend_mode,
                                |_, alpha| alpha,
                            );
                        }
                        Some(crate::MaskCoverage::Uniform(coverage)) => {
                            if *coverage > 0 {
                                composite_flat_row(
                                    row,
                                    source_row,
                                    layer.opacity,
                                    layer.blend_mode,
                                    |_, alpha| {
                                        crate::layer_mask::masked_alpha(
                                            alpha, *coverage,
                                        )
                                    },
                                );
                            }
                        }
                        Some(crate::MaskCoverage::Dense(values)) => {
                            let mask_row =
                                &values[start / 4..(start + stride) / 4];
                            composite_flat_row(
                                row,
                                source_row,
                                layer.opacity,
                                layer.blend_mode,
                                |column, alpha| {
                                    crate::layer_mask::masked_alpha(
                                        alpha,
                                        mask_row[column],
                                    )
                                },
                            );
                        }
                    }
                }
                continue;
            }
            clipped_row.resize(stride / 4, [0; 4]);
            let clips = &clips[..clip_count];
            if layer.has_effective_mask()
                || clips.iter().any(|clip| clip.has_effective_mask())
            {
                composite_clipped_rows::<true>(
                    layer,
                    clips,
                    &mut out,
                    &mut clipped_row,
                    self.width,
                    r,
                );
            } else {
                composite_clipped_rows::<false>(
                    layer,
                    clips,
                    &mut out,
                    &mut clipped_row,
                    self.width,
                    r,
                );
            }
        }
        out
    }
}

fn composite_clipped_rows<const MASKED: bool>(
    layer: &Layer,
    clips: &[&Layer],
    output: &mut [u8],
    clipped_row: &mut [[u8; 4]],
    width: u32,
    bounds: PixelRect,
) {
    let stride = (bounds.x1 - bounds.x0) as usize * 4;
    for (row_index, row) in output.chunks_exact_mut(stride).enumerate() {
        let start = ((bounds.y0 as usize + row_index) * width as usize
            + bounds.x0 as usize)
            * 4;
        let source_row = layer.pixels[start..start + stride].as_chunks::<4>().0;
        clipped_row.fill([0; 4]);
        for clip in clips {
            let clip_row =
                clip.pixels[start..start + stride].as_chunks::<4>().0;
            for (column, (((color, source), base), backdrop)) in clipped_row
                .iter_mut()
                .zip(clip_row)
                .zip(source_row)
                .zip(row.as_chunks::<4>().0)
                .enumerate()
            {
                let source_alpha = if MASKED {
                    clip.mask_alpha(start / 4 + column, source[3])
                } else {
                    source[3]
                };
                let base_alpha = if MASKED {
                    layer.mask_alpha(start / 4 + column, base[3])
                } else {
                    base[3]
                };
                if source_alpha == 0 || base_alpha == 0 {
                    continue;
                }
                if color[3] == 0 {
                    *color = [base[0], base[1], base[2], 255];
                    if layer.blend_mode != BlendMode::Normal && backdrop[3] > 0
                    {
                        let backdrop_alpha = backdrop[3] as f32 / 255.;
                        for channel in 0..3 {
                            let source = base[channel] as f32 / 255.;
                            let blended = layer.blend_mode.channel(
                                backdrop[channel] as f32 / 255.,
                                source,
                            );
                            color[channel] = ((source * (1. - backdrop_alpha)
                                + blended * backdrop_alpha)
                                * 255.)
                                .round()
                                .clamp(0., 255.)
                                as u8;
                        }
                    }
                }
                composite_opaque(
                    color,
                    [source[0], source[1], source[2]],
                    source_alpha as f32 / 255. * clip.opacity,
                    clip.blend_mode,
                );
            }
        }
        for (column, (destination, source)) in
            row.as_chunks_mut::<4>().0.iter_mut().zip(source_row).enumerate()
        {
            let alpha = if MASKED {
                layer.mask_alpha(start / 4 + column, source[3])
            } else {
                source[3]
            };
            if alpha == 0 {
                continue;
            }
            let color = clipped_row[column];
            if color[3] > 0 {
                composite_blend(
                    destination,
                    [color[0], color[1], color[2]],
                    alpha as f32 / 255. * layer.opacity,
                    BlendMode::Normal,
                );
            } else {
                composite_blend(
                    destination,
                    [source[0], source[1], source[2]],
                    alpha as f32 / 255. * layer.opacity,
                    layer.blend_mode,
                );
            }
        }
    }
}

pub(crate) fn composite_flat_row(
    row: &mut [u8],
    source_row: &[[u8; 4]],
    opacity: f32,
    mode: BlendMode,
    alpha: impl Fn(usize, u8) -> u8,
) {
    for (column, (destination, source)) in
        row.as_chunks_mut::<4>().0.iter_mut().zip(source_row).enumerate()
    {
        let alpha = alpha(column, source[3]);
        if alpha == 0 {
            continue;
        }
        composite_blend(
            destination,
            [source[0], source[1], source[2]],
            alpha as f32 / 255. * opacity,
            mode,
        );
    }
}

#[cfg(test)]
#[path = "compositing_tests.rs"]
mod compositing_tests;

pub fn composite_over(dst: &mut [u8], rgb: [u8; 3], alpha: f32) {
    let sa = alpha.clamp(0., 1.);
    if sa == 0. {
        return;
    }
    if sa == 1. {
        dst[..3].copy_from_slice(&rgb);
        dst[3] = 255;
        return;
    }
    let da = dst[3] as f32 / 255.;
    let a = sa + da * (1. - sa);
    for i in 0..3 {
        dst[i] = ((rgb[i] as f32 * sa + dst[i] as f32 * da * (1. - sa)) / a)
            .round() as u8;
    }
    dst[3] = (a * 255.).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing_uses_visible_effective_alpha_but_allows_locked_layer_selection()
     {
        let mut document = Document::new(2, 2);
        assert_eq!(document.layer_at(0, 0), Some(0));
        Arc::make_mut(&mut document.layers[1].pixels)[3] = 1;
        document.layers[1].locked = true;
        assert_eq!(document.layer_at(0, 0), Some(1));
        assert_eq!(document.layer_at(1, 1), Some(0));
        document.layers[1].opacity = 0.;
        assert_eq!(document.layer_at(0, 0), Some(0));
        document.layers[1].opacity = 1.;
        document.layers[1].visible = false;
        assert_eq!(document.layer_at(0, 0), Some(0));
        document.layers[0].visible = false;
        assert_eq!(document.layer_at(0, 0), None);
        assert_eq!(document.layer_at(u32::MAX, 0), None);
    }
    #[test]
    fn layer_order_opacity_and_visibility() {
        let mut d = Document::new(1, 1);
        d.layers[1].pixels = Arc::new(vec![255, 0, 0, 255]);
        d.layers[1].opacity = 0.5;
        assert_eq!(d.composite(), vec![255, 128, 128, 255]);
        d.layers[1].visible = false;
        assert_eq!(d.composite(), vec![255; 4]);
    }
    #[test]
    fn snapshot_pixels_are_copy_on_write() {
        let mut d = Document::new(2, 2);
        let before = d.clone();
        Arc::make_mut(&mut d.layers[1].pixels)[0] = 123;
        assert_eq!(before.layers[1].pixels[0], 0);
        assert_eq!(d.layers[1].pixels[0], 123);
    }
}
