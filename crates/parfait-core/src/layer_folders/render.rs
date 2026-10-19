use super::{FolderLayout, LayerKind};
use crate::{
    BlendMode, Document, PixelRect,
    blend::{composite_blend, composite_opaque},
};

struct RowComposite<'a> {
    document: &'a Document,
    layout: &'a FolderLayout,
    folders: &'a [Vec<u8>],
    selection: u64,
    start: usize,
    stride: usize,
}

impl RowComposite<'_> {
    fn source(&self, index: usize) -> &[u8] {
        let layer = &self.document.layers[index];
        if layer.kind == LayerKind::Folder {
            &self.folders[index]
        } else {
            &layer.pixels[self.start..self.start + self.stride]
        }
    }

    fn draw(&self, parent: usize, output: &mut [u8]) {
        output.fill(0);
        for (index, base) in self.document.layers.iter().enumerate() {
            if self.layout.parents[index] != parent
                || base.clipped
                || !base.visible
                || base.opacity == 0.
                || self.selection & (1 << index) == 0
            {
                continue;
            }
            let mut clips = [0; 64];
            let mut count = 0;
            for (clip, layer) in self.document.layers.iter().enumerate().skip(index + 1) {
                if self.layout.parents[clip] != parent {
                    continue;
                }
                if !layer.clipped {
                    break;
                }
                if layer.visible && layer.opacity > 0. && self.selection & (1 << clip) != 0 {
                    clips[count] = clip;
                    count += 1;
                }
            }
            let source = self.source(index).as_chunks::<4>().0;
            if count == 0 {
                if base.has_effective_mask() {
                    crate::document::composite_flat_row(
                        output,
                        source,
                        base.opacity,
                        base.blend_mode,
                        |column, alpha| base.mask_alpha(self.start / 4 + column, alpha),
                    );
                } else {
                    crate::document::composite_flat_row(
                        output,
                        source,
                        base.opacity,
                        base.blend_mode,
                        |_, alpha| alpha,
                    );
                }
                continue;
            }
            for (column, (destination, pixel)) in output
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(source)
                .enumerate()
            {
                let alpha = base.mask_alpha(self.start / 4 + column, pixel[3]);
                if alpha == 0 {
                    continue;
                }
                let mut color = [pixel[0], pixel[1], pixel[2], 255];
                let mut clipped = false;
                for clip in &clips[..count] {
                    let layer = &self.document.layers[*clip];
                    let pixel = &self.source(*clip)[column * 4..column * 4 + 4];
                    let alpha = layer.mask_alpha(self.start / 4 + column, pixel[3]);
                    if alpha == 0 {
                        continue;
                    }
                    if !clipped && base.blend_mode != BlendMode::Normal && destination[3] > 0 {
                        let backdrop_alpha = f32::from(destination[3]) / 255.;
                        for channel in 0..3 {
                            let source = f32::from(color[channel]) / 255.;
                            let blended = base
                                .blend_mode
                                .channel(f32::from(destination[channel]) / 255., source);
                            color[channel] = ((source * (1. - backdrop_alpha)
                                + blended * backdrop_alpha)
                                * 255.)
                                .round()
                                .clamp(0., 255.) as u8;
                        }
                    }
                    clipped = true;
                    composite_opaque(
                        &mut color,
                        [pixel[0], pixel[1], pixel[2]],
                        f32::from(alpha) / 255. * layer.opacity,
                        layer.blend_mode,
                    );
                }
                composite_blend(
                    destination,
                    [color[0], color[1], color[2]],
                    f32::from(alpha) / 255. * base.opacity,
                    if clipped {
                        BlendMode::Normal
                    } else {
                        base.blend_mode
                    },
                );
            }
        }
    }
}

fn folder_rows(document: &Document, stride: usize) -> Vec<Vec<u8>> {
    document
        .layers
        .iter()
        .map(|layer| {
            if layer.kind == LayerKind::Folder {
                vec![0; stride]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn fill_folders(
    document: &Document,
    layout: &FolderLayout,
    rows: &mut [Vec<u8>],
    selection: u64,
    start: usize,
    stride: usize,
) {
    for (index, layer) in document.layers.iter().enumerate() {
        if layer.kind != LayerKind::Folder {
            continue;
        }
        let (children, folder) = rows.split_at_mut(index);
        RowComposite {
            document,
            layout,
            folders: children,
            selection,
            start,
            stride,
        }
        .draw(index, &mut folder[0]);
    }
}

impl Document {
    pub(crate) fn composite_folder_region(&self, rect: PixelRect, selection: u64) -> Vec<u8> {
        let rect = rect.clipped(self.width, self.height);
        let stride = (rect.x1 - rect.x0) as usize * 4;
        let mut output = vec![0; stride * (rect.y1 - rect.y0) as usize];
        if stride == 0 {
            return output;
        }
        let Some(layout) = self.folder_layout() else {
            return output;
        };
        let mut folders = folder_rows(self, stride);
        for (row, destination) in output.chunks_exact_mut(stride).enumerate() {
            let start = ((rect.y0 as usize + row) * self.width as usize + rect.x0 as usize) * 4;
            fill_folders(self, &layout, &mut folders, selection, start, stride);
            RowComposite {
                document: self,
                layout: &layout,
                folders: &folders,
                selection,
                start,
                stride,
            }
            .draw(64, destination);
        }
        output
    }

    pub(crate) fn folder_layer_at(&self, column: u32, row: u32) -> Option<usize> {
        let layout = self.folder_layout()?;
        let start = ((row * self.width + column) * 4) as usize;
        let mut folders = folder_rows(self, 4);
        fill_folders(self, &layout, &mut folders, u64::MAX, start, 4);
        let composite = RowComposite {
            document: self,
            layout: &layout,
            folders: &folders,
            selection: u64::MAX,
            start,
            stride: 4,
        };
        self.layers
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, layer)| {
                if layer.kind != LayerKind::Raster {
                    return None;
                }
                let mut ancestor = index;
                while ancestor != 64 {
                    let target = &self.layers[ancestor];
                    let base = self.clipping_base(ancestor)?;
                    let base_layer = &self.layers[base];
                    if !target.visible
                        || target.opacity == 0.
                        || target.mask_alpha(start / 4, composite.source(ancestor)[3]) == 0
                        || !base_layer.visible
                        || base_layer.opacity == 0.
                        || base_layer.mask_alpha(start / 4, composite.source(base)[3]) == 0
                    {
                        return None;
                    }
                    ancestor = layout.parents[ancestor];
                }
                Some(index)
            })
    }
}
