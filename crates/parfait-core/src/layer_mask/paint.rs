use super::MaskCoverage;
use crate::{Brush, BrushOperation, Document, OpacityMode, PixelRect, Point, brush::Dab};
use std::sync::Arc;

const SIDE: usize = 64;
const CELLS: usize = SIDE * SIDE;

pub(crate) struct MaskPaint {
    baseline: Option<MaskCoverage>,
    width: u32,
    height: u32,
    columns: usize,
    tiles: Vec<Option<Box<[f32; CELLS]>>>,
    allocated: usize,
}

enum Destination<'a> {
    Shared(&'a mut MaskCoverage),
    Unique(&'a mut Vec<u8>),
}

impl<'a> Destination<'a> {
    fn value(&self, index: usize) -> u8 {
        match self {
            Self::Shared(coverage) => coverage.value(index),
            Self::Unique(values) => values[index],
        }
    }

    fn write(self, index: usize, value: u8, count: usize) -> (Self, bool) {
        if self.value(index) == value {
            return (self, false);
        }
        let values = match self {
            Self::Shared(coverage) => {
                if let MaskCoverage::Uniform(initial) = coverage {
                    *coverage = MaskCoverage::Dense(Arc::new(vec![*initial; count]));
                }
                let MaskCoverage::Dense(values) = coverage else {
                    unreachable!()
                };
                Arc::make_mut(values)
            }
            Self::Unique(values) => values,
        };
        values[index] = value;
        (Self::Unique(values), true)
    }
}

impl MaskPaint {
    pub fn new(document: &Document, brush: &Brush) -> Self {
        Self {
            baseline: (brush.opacity_mode == OpacityMode::PerStroke).then(|| {
                document.layers[document.active]
                    .mask
                    .as_ref()
                    .unwrap()
                    .coverage
                    .clone()
            }),
            width: document.width,
            height: document.height,
            columns: (document.width as usize).div_ceil(SIDE),
            tiles: Vec::new(),
            allocated: 0,
        }
    }

    pub fn capacity_bytes(&self) -> usize {
        self.tiles.capacity() * std::mem::size_of::<Option<Box<[f32; CELLS]>>>()
            + self.allocated * std::mem::size_of::<[f32; CELLS]>()
    }

    pub fn stamp(
        &mut self,
        document: &mut Document,
        brush: &Brush,
        point: Point,
    ) -> Option<PixelRect> {
        if document.width != self.width || document.height != self.height {
            return None;
        }
        let dab = Dab::new_mask(document, brush, point)?;
        if !dab.has_strength(brush) {
            return None;
        }
        let mask = document.layers[document.active]
            .mask
            .as_mut()
            .filter(|mask| mask.enabled)?;
        let erase = brush.operation() == BrushOperation::Erase;
        let limit = dab.opacity_limit(brush);
        let mut destination = Destination::Shared(&mut mask.coverage);
        let mut changed = false;
        for row in dab.bounds.y0 as usize..dab.bounds.y1 as usize {
            for column in dab.bounds.x0 as usize..dab.bounds.x1 as usize {
                let limit = crate::selection::strength(
                    document.selection.as_ref(),
                    row * self.width as usize + column,
                    limit,
                );
                if limit <= 0. {
                    continue;
                }
                let index = row * self.width as usize + column;
                let current = destination.value(index);
                if (erase && current == 0) || (!erase && current == 255) {
                    continue;
                }
                let deposit = dab.coverage(brush, column as u32, row as u32) * brush.flow;
                if deposit <= 0. {
                    continue;
                }
                let (initial, amount) = if let Some(baseline) = &self.baseline {
                    if self.tiles.is_empty() {
                        self.tiles =
                            vec![None; self.columns * (self.height as usize).div_ceil(SIDE)];
                    }
                    let tile = self.tiles[row / SIDE * self.columns + column / SIDE]
                        .get_or_insert_with(|| {
                            self.allocated += 1;
                            Box::new([0.; CELLS])
                        });
                    let contribution = &mut tile[(row % SIDE) * SIDE + column % SIDE];
                    let next = (*contribution + (limit - *contribution).max(0.) * deposit)
                        .min(limit.max(*contribution));
                    *contribution = next;
                    (baseline.value(index), next)
                } else {
                    (current, deposit * limit)
                };
                let result = if erase {
                    (f32::from(initial) * (1. - amount)).round() as u8
                } else {
                    (f32::from(initial) + f32::from(255 - initial) * amount).round() as u8
                };
                let (next, written) =
                    destination.write(index, result, (self.width * self.height) as usize);
                destination = next;
                changed |= written;
            }
        }
        if changed {
            document.touch();
            Some(dab.bounds)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
