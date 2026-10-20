use crate::{
    ContentBox, Document, LayerId, LayerKind, MaskCoverage, PixelRect,
};
use std::sync::Arc;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Top,
    CenterY,
    Bottom,
    Left,
    CenterX,
    Right,
    DistributeTop,
    DistributeCenterY,
    DistributeBottom,
    DistributeLeft,
    DistributeCenterX,
    DistributeRight,
    SpaceY,
    SpaceX,
}
impl Align {
    fn horizontal(self) -> bool {
        matches!(
            self,
            Self::Left
                | Self::CenterX
                | Self::Right
                | Self::DistributeLeft
                | Self::DistributeCenterX
                | Self::DistributeRight
                | Self::SpaceX
        )
    }
    fn distribution(self) -> bool {
        !matches!(
            self,
            Self::Top
                | Self::CenterY
                | Self::Bottom
                | Self::Left
                | Self::CenterX
                | Self::Right
        )
    }
    fn factor(self) -> f64 {
        match self {
            Self::CenterX
            | Self::CenterY
            | Self::DistributeCenterX
            | Self::DistributeCenterY => 0.5,
            Self::Bottom
            | Self::Right
            | Self::DistributeBottom
            | Self::DistributeRight => 1.,
            _ => 0.,
        }
    }
}
impl Document {
    pub fn layer_pixel_bounds(&self, index: usize) -> Option<PixelRect> {
        let layer = self.layers.get(index)?;
        if layer.kind != LayerKind::Raster {
            return None;
        }
        crate::image_edit::bounds(self.width, self.height, |i| {
            layer.effective_alpha(i) > 0
        })
    }
    pub fn alignment_bounds(
        &self,
        selection: u64,
    ) -> Result<Vec<(LayerId, PixelRect)>, &'static str> {
        if !self.validate()
            || selection == 0
            || (self.layers.len() < 64 && selection >> self.layers.len() != 0)
        {
            return Err("정렬할 레이어를 선택해 주세요.");
        }
        let mut records = Vec::new();
        for (index, layer) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(i, _)| selection & (1 << i) != 0)
        {
            if layer.kind != LayerKind::Raster {
                return Err("현재 정렬은 래스터 레이어를 대상으로 합니다.");
            }
            if !self.layer_ancestors_unlocked(index) {
                return Err("선택 레이어와 부모 폴더의 잠금을 해제해 주세요.");
            }
            if let Some(rect) = self.layer_pixel_bounds(index) {
                records.push((layer.id, rect));
            }
        }
        Ok(records)
    }
    pub fn alignment_plan(
        &self,
        selection: u64,
        mode: Align,
        reference: Option<PixelRect>,
    ) -> Result<Vec<(LayerId, [i32; 2])>, &'static str> {
        self.plan_alignment_bounds(
            &self.alignment_bounds(selection)?,
            mode,
            reference,
        )
    }
    pub fn plan_alignment_bounds(
        &self,
        bounds: &[(LayerId, PixelRect)],
        mode: Align,
        reference: Option<PixelRect>,
    ) -> Result<Vec<(LayerId, [i32; 2])>, &'static str> {
        self.plan_content_alignment(
            &bounds
                .iter()
                .map(|(id, r)| (*id, ContentBox::from(*r)))
                .collect::<Vec<_>>(),
            mode,
            reference,
        )
    }
    pub fn content_alignment_bounds(
        &self,
        selection: u64,
        text_ink: bool,
        vector_center: bool,
    ) -> Result<Vec<(LayerId, ContentBox)>, &'static str> {
        if !self.validate()
            || selection == 0
            || (self.layers.len() < 64 && selection >> self.layers.len() != 0)
        {
            return Err("정렬할 레이어를 선택해 주세요.");
        }
        let mut records = Vec::new();
        for (i, l) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(i, _)| selection & (1 << i) != 0)
        {
            if l.kind != LayerKind::Raster || !self.layer_ancestors_unlocked(i)
            {
                return Err(
                    "잠금이 풀린 래스터·텍스트·벡터 레이어를 선택해 주세요.",
                );
            }
            let bounds = if let Some(node) = self.native_content(i) {
                node.bounds(text_ink, vector_center)
            } else {
                self.layer_pixel_bounds(i).map(ContentBox::from)
            };
            if let Some(bounds) = bounds {
                records.push((l.id, bounds));
            }
        }
        Ok(records)
    }
    pub fn plan_content_alignment(
        &self,
        bounds: &[(LayerId, ContentBox)],
        mode: Align,
        reference: Option<PixelRect>,
    ) -> Result<Vec<(LayerId, [i32; 2])>, &'static str> {
        if bounds.len() > 64
            || bounds.iter().enumerate().any(|(i, (id, r))| {
                !r.validate()
                    || self.layer_index(*id).is_none()
                    || bounds[..i].iter().any(|(prior, _)| prior == id)
            })
        {
            return Err("레이어 경계가 유효하지 않습니다.");
        }
        if reference.is_some_and(|r| {
            r.x0 >= r.x1
                || r.y0 >= r.y1
                || r.x1 > self.width
                || r.y1 > self.height
        }) {
            return Err("정렬 기준 범위가 유효하지 않습니다.");
        }
        let horizontal = mode.horizontal();
        let factor = mode.factor();
        let mut records: Vec<_> = bounds
            .iter()
            .map(|(id, rect)| {
                let (start, size) = if horizontal {
                    (rect.min[0], rect.max[0] - rect.min[0])
                } else {
                    (rect.min[1], rect.max[1] - rect.min[1])
                };
                (*id, f64::from(start), f64::from(size))
            })
            .collect();
        let minimum = if mode.distribution() {
            3
        } else if reference.is_some() {
            1
        } else {
            2
        };
        if records.len() < minimum {
            return Err(
                "정렬에는 내용이 있는 레이어가 필요하며 분포에는 3개 이상이 필요합니다.",
            );
        }
        let bounds = records.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(low, high), (_, start, size)| {
                (low.min(*start), high.max(start + size))
            },
        );
        let (low, high) = reference.map_or(bounds, |r| {
            if horizontal {
                (f64::from(r.x0), f64::from(r.x1))
            } else {
                (f64::from(r.y0), f64::from(r.y1))
            }
        });
        records.sort_by(|a, b| {
            (a.1 + a.2 * factor).total_cmp(&(b.1 + b.2 * factor))
        });
        let first = records[0].1 + records[0].2 * factor;
        let last = records.last().unwrap();
        let last_anchor = last.1 + last.2 * factor;
        let (first, last) = if reference.is_some() {
            (low + records[0].2 * factor, high - last.2 * (1. - factor))
        } else {
            (first, last_anchor)
        };
        let spacing = (high - low - records.iter().map(|v| v.2).sum::<f64>())
            / (records.len() - 1).max(1) as f64;
        let mut cursor = low;
        let count = records.len();
        let mut plan = Vec::with_capacity(count);
        for (i, (id, start, size)) in records.into_iter().enumerate() {
            let destination = if matches!(mode, Align::SpaceX | Align::SpaceY) {
                let p = cursor;
                cursor += size + spacing;
                p
            } else if mode.distribution() {
                first + (last - first) * i as f64 / (count - 1) as f64
                    - size * factor
            } else {
                low + (high - low) * factor - size * factor
            };
            let delta = (destination - start).round() as i32;
            plan.push((id, if horizontal { [delta, 0] } else { [0, delta] }));
        }
        Ok(plan)
    }
    pub fn translate_layer_pixels(
        &mut self,
        plan: &[(LayerId, [i32; 2])],
    ) -> bool {
        if !self.validate()
            || plan.len() > 64
            || plan.iter().enumerate().any(|(i, (id, _))| {
                plan[..i].iter().any(|(previous, _)| previous == id)
                    || self.layer_index(*id).is_none_or(|index| {
                        self.layers[index].kind != LayerKind::Raster
                            || !self.layer_ancestors_unlocked(index)
                    })
            })
        {
            return false;
        }
        let mut next = self.clone();
        let mut changed = false;
        for (id, [dx, dy]) in plan {
            if *dx == 0 && *dy == 0 {
                continue;
            }
            let index = self.layer_index(*id).unwrap();
            let source = &self.layers[index];
            let mut pixels = vec![0; source.pixels.len()];
            let mut mask = source
                .mask
                .as_ref()
                .map(|_| vec![255; (self.width * self.height) as usize]);
            for y in 0..self.height {
                for x in 0..self.width {
                    let sx = i64::from(x) - i64::from(*dx);
                    let sy = i64::from(y) - i64::from(*dy);
                    if sx >= 0
                        && sy >= 0
                        && sx < i64::from(self.width)
                        && sy < i64::from(self.height)
                    {
                        let dest = (y * self.width + x) as usize;
                        let src =
                            sy as usize * self.width as usize + sx as usize;
                        pixels[dest * 4..dest * 4 + 4].copy_from_slice(
                            &source.pixels[src * 4..src * 4 + 4],
                        );
                        if let Some(values) = &mut mask {
                            values[dest] = source
                                .mask
                                .as_ref()
                                .unwrap()
                                .coverage
                                .value(src);
                        }
                    }
                }
            }
            if let Some(node) = self.native_content(index) {
                let Ok(node) = node.transformed(crate::Affine::translation(
                    *dx as f32, *dy as f32,
                )) else {
                    return false;
                };
                let Ok(rendered) = node.render(self.width, self.height) else {
                    return false;
                };
                pixels = (*rendered).clone();
                let position = next
                    .contents
                    .binary_search_by_key(&id.0, |n| n.layer.0)
                    .unwrap();
                next.contents[position] = node;
                changed = true;
            }
            if pixels != *source.pixels {
                next.layers[index].pixels = Arc::new(pixels);
                changed = true;
            }
            if let Some(values) = mask {
                let first = values[0];
                let coverage = if values.iter().all(|v| *v == first) {
                    MaskCoverage::Uniform(first)
                } else {
                    MaskCoverage::Dense(Arc::new(values))
                };
                if coverage != source.mask.as_ref().unwrap().coverage {
                    next.layers[index].mask.as_mut().unwrap().coverage =
                        coverage;
                    changed = true;
                }
            }
        }
        if !changed || !next.validate() {
            return false;
        }
        next.touch();
        *self = next;
        true
    }
}
