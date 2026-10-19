use super::*;

struct BackdropRow<'a> {
    document: &'a Document,
    layout: &'a FolderLayout,
    folders: &'a mut [Vec<u8>],
    selection: u64,
    start: usize,
    stride: usize,
}

impl BackdropRow<'_> {
    fn prepare(&mut self, index: usize) {
        if self.document.layers[index].kind != LayerKind::Folder {
            return;
        }
        let mut row = std::mem::take(&mut self.folders[index]);
        row.fill(0);
        self.draw(index, &mut row);
        self.folders[index] = row;
    }

    fn draw(&mut self, parent: usize, output: &mut [u8]) {
        for index in 0..self.document.layers.len() {
            let base = &self.document.layers[index];
            if self.layout.parents[index] != parent
                || base.clipped
                || !base.visible
                || base.opacity == 0.
                || self.selection & (1 << index) == 0
            {
                continue;
            }
            if base.blend_mode == BlendMode::Through {
                if base.opacity == 1. && !base.has_effective_mask() {
                    self.draw(index, output);
                } else {
                    let mut before = std::mem::take(&mut self.folders[index]);
                    before.copy_from_slice(output);
                    self.draw(index, output);
                    interpolate(base, self.start / 4, &before, output);
                    self.folders[index] = before;
                }
                continue;
            }
            self.prepare(index);
            let mut clips = [0; 64];
            let mut count = 0;
            for clip in index + 1..self.document.layers.len() {
                let layer = &self.document.layers[clip];
                if self.layout.parents[clip] != parent {
                    continue;
                }
                if !layer.clipped {
                    break;
                }
                if layer.visible && layer.opacity > 0. && self.selection & (1 << clip) != 0 {
                    self.prepare(clip);
                    clips[count] = clip;
                    count += 1;
                }
            }
            RowComposite {
                document: self.document,
                layout: self.layout,
                folders: self.folders,
                selection: self.selection,
                start: self.start,
                stride: self.stride,
            }
            .draw_group(index, &clips[..count], output);
        }
    }
}

fn interpolate(layer: &crate::Layer, start: usize, before: &[u8], after: &mut [u8]) {
    for (column, (before, after)) in before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.as_chunks_mut::<4>().0)
        .enumerate()
    {
        let factor = layer.opacity * f32::from(layer.mask_alpha(start + column, 255)) / 255.;
        if factor == 1. {
            continue;
        }
        if factor == 0. {
            *after = *before;
            continue;
        }
        let before_alpha = f32::from(before[3]) / 255.;
        let after_alpha = f32::from(after[3]) / 255.;
        let alpha = before_alpha + (after_alpha - before_alpha) * factor;
        if alpha == 0. {
            *after = [0; 4];
            continue;
        }
        for channel in 0..3 {
            let previous = f32::from(before[channel]) * before_alpha;
            let current = f32::from(after[channel]) * after_alpha;
            after[channel] = ((previous + (current - previous) * factor) / alpha)
                .round()
                .clamp(0., 255.) as u8;
        }
        after[3] = (alpha * 255.).round().clamp(0., 255.) as u8;
    }
}

pub(super) fn compose(
    document: &Document,
    layout: &FolderLayout,
    folders: &mut [Vec<u8>],
    rect: PixelRect,
    selection: u64,
    output: &mut [u8],
) {
    let stride = (rect.x1 - rect.x0) as usize * 4;
    let mut compositor = BackdropRow {
        document,
        layout,
        folders,
        selection,
        start: 0,
        stride,
    };
    for (row, destination) in output.chunks_exact_mut(stride).enumerate() {
        compositor.start =
            ((rect.y0 as usize + row) * document.width as usize + rect.x0 as usize) * 4;
        compositor.draw(64, destination);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_and_folder_row_capacity_stays_within_one_mebibyte_at_maximum_width() {
        let mut document = Document::new_with_paper(4096, 1);
        document.create_layer_folder("Pass".into()).unwrap();
        document.layers[2].blend_mode = BlendMode::Through;
        while document.layers.len() < 64 {
            document
                .create_solid_layer([30, 80, 150], false, "Fill".into())
                .unwrap();
        }
        let mut rows = folder_rows(&document, 4096 * 4);
        assert_eq!(rows.iter().map(Vec::len).sum::<usize>(), 63 * 4096 * 4);
        assert!(rows.iter().map(Vec::capacity).sum::<usize>() <= 1024 * 1024);
        let identities: Vec<_> = rows
            .iter()
            .map(|row| (row.as_ptr(), row.capacity()))
            .collect();
        let mut output = vec![0; 4096 * 4];
        compose(
            &document,
            &document.folder_layout().unwrap(),
            &mut rows,
            PixelRect {
                x0: 0,
                y0: 0,
                x1: 4096,
                y1: 1,
            },
            u64::MAX,
            &mut output,
        );
        assert_eq!(
            identities,
            rows.iter()
                .map(|row| (row.as_ptr(), row.capacity()))
                .collect::<Vec<_>>()
        );
        assert!(
            output
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [30, 80, 150, 255])
        );
    }

    #[test]
    fn deepest_pass_groups_reuse_bounded_row_storage() {
        let mut document = Document::new(4096, 1);
        document.layers[1].pixels = std::sync::Arc::new([255, 0, 0, 255].repeat(4096));
        while document.layers.len() < 64 {
            document
                .group_layers_in_folder(1 << document.active, "Nested".into())
                .unwrap();
            document.layers[document.active].blend_mode = BlendMode::Through;
            document.layers[document.active].opacity = 0.5;
        }
        assert!(document.validate());
        let mut folders = folder_rows(&document, 4096 * 4);
        let identities: Vec<_> = folders
            .iter()
            .map(|row| (row.as_ptr(), row.capacity()))
            .collect();
        assert_eq!(folders.iter().map(Vec::len).sum::<usize>(), 62 * 4096 * 4);
        assert!(folders.iter().map(Vec::len).sum::<usize>() <= 1024 * 1024);
        let mut output = vec![0; 4096 * 4];
        compose(
            &document,
            &document.folder_layout().unwrap(),
            &mut folders,
            PixelRect {
                x0: 0,
                y0: 0,
                x1: 4096,
                y1: 1,
            },
            u64::MAX,
            &mut output,
        );
        assert_eq!(
            identities,
            folders
                .iter()
                .map(|row| (row.as_ptr(), row.capacity()))
                .collect::<Vec<_>>()
        );
        assert!(output.iter().all(|byte| *byte == 255));
    }
}
