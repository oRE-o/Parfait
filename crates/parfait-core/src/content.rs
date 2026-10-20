//! Editable text/vector data with a raster cache in the associated layer.
//! Fonts are embedded data; rendering never consults installed fonts or services.
use crate::{Document, LayerId, LayerKind};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
mod raster;
#[cfg(test)]
mod tests;

pub const MAX_CONTENT_POINTS: usize = 65_536;
pub const MAX_CONTENT_PATHS: usize = 256;
pub const MAX_CONTENT_FONT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine(pub [f32; 6]);
impl Default for Affine {
    fn default() -> Self {
        Self([1., 0., 0., 1., 0., 0.])
    }
}
impl Affine {
    pub fn translation(x: f32, y: f32) -> Self {
        Self([1., 0., 0., 1., x, y])
    }
    pub fn point(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, tx, ty] = self.0;
        [a * x + c * y + tx, b * x + d * y + ty]
    }
    pub fn then(self, next: Self) -> Self {
        let [a, b, c, d, x, y] = self.0;
        let [e, f, g, h, u, v] = next.0;
        Self([
            e * a + g * b,
            f * a + h * b,
            e * c + g * d,
            f * c + h * d,
            e * x + g * y + u,
            f * x + h * y + v,
        ])
    }
    pub fn validate(self) -> bool {
        self.0.iter().all(|v| v.is_finite())
            && self.0[..4].iter().all(|v| v.abs() <= 4096.)
            && self.0[4..].iter().all(|v| v.abs() <= 1_000_000.)
            && (self.0[0] * self.0[3] - self.0[1] * self.0[2]).abs() > 1e-12
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentBox {
    pub min: [f32; 2],
    pub max: [f32; 2],
}
impl ContentBox {
    pub fn validate(self) -> bool {
        self.min
            .into_iter()
            .chain(self.max)
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            && self.min[0] <= self.max[0]
            && self.min[1] <= self.max[1]
    }
    pub fn transformed(self, m: Affine) -> Self {
        Self::points(&[
            m.point(self.min),
            m.point([self.max[0], self.min[1]]),
            m.point(self.max),
            m.point([self.min[0], self.max[1]]),
        ])
        .unwrap()
    }
    pub(crate) fn points(points: &[[f32; 2]]) -> Option<Self> {
        let first = *points.first()?;
        let mut bounds = Self { min: first, max: first };
        for p in &points[1..] {
            for (c, value) in p.iter().enumerate() {
                bounds.min[c] = bounds.min[c].min(*value);
                bounds.max[c] = bounds.max[c].max(*value);
            }
        }
        Some(bounds)
    }
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            min: [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])],
            max: [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])],
        }
    }
}
impl From<crate::PixelRect> for ContentBox {
    fn from(r: crate::PixelRect) -> Self {
        Self {
            min: [r.x0 as f32, r.y0 as f32],
            max: [r.x1 as f32, r.y1 as f32],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VectorPath {
    #[serde(deserialize_with = "deserialize_points")]
    pub points: Arc<Vec<[f32; 2]>>,
    pub color: [u8; 4],
    pub width: f32,
    pub closed: bool,
    pub fill: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VectorContent {
    #[serde(deserialize_with = "deserialize_paths")]
    pub paths: Vec<VectorPath>,
    pub matrix: Affine,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(deserialize_with = "deserialize_text")]
    pub text: String,
    #[serde(deserialize_with = "deserialize_font")]
    pub font: Arc<Vec<u8>>,
    pub size: f32,
    pub color: [u8; 4],
    pub matrix: Affine,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayerContent {
    Vector(VectorContent),
    Text(TextContent),
}

#[derive(Clone, Copy, Debug)]
struct ContentStats {
    wire: u64,
    logical: u64,
    layout: Option<ContentBox>,
    ink: Option<ContentBox>,
    center: Option<ContentBox>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentNode {
    pub layer: LayerId,
    body: Arc<LayerContent>,
    #[serde(skip)]
    stats: OnceLock<Result<ContentStats, &'static str>>,
}
impl PartialEq for ContentNode {
    fn eq(&self, other: &Self) -> bool {
        self.layer == other.layer && self.body == other.body
    }
}
impl ContentNode {
    pub fn new(
        layer: LayerId,
        body: LayerContent,
    ) -> Result<Self, &'static str> {
        let node = Self { layer, body: Arc::new(body), stats: OnceLock::new() };
        node.statistics()?;
        Ok(node)
    }
    pub fn body(&self) -> &Arc<LayerContent> {
        &self.body
    }
    fn statistics(&self) -> Result<&ContentStats, &'static str> {
        self.stats
            .get_or_init(|| raster::statistics(&self.body))
            .as_ref()
            .map_err(|e| *e)
    }
    pub fn validate(&self) -> bool {
        self.statistics().is_ok()
    }
    pub fn logical_bytes(&self) -> u64 {
        self.statistics().map_or(u64::MAX, |s| s.logical)
    }
    pub fn encoded_bytes(&self) -> Option<u64> {
        self.statistics().ok()?.wire.checked_add(8)
    }
    pub fn bounds(
        &self,
        text_ink: bool,
        vector_center: bool,
    ) -> Option<ContentBox> {
        let s = self.statistics().ok()?;
        match self.body.as_ref() {
            LayerContent::Text(_) => {
                if text_ink {
                    s.ink
                } else {
                    s.layout
                }
            }
            LayerContent::Vector(_) => {
                if vector_center {
                    s.center
                } else {
                    s.ink
                }
            }
        }
    }
    pub fn transformed(&self, m: Affine) -> Result<Self, &'static str> {
        Self::new(self.layer, self.body.transformed(m))
    }
    pub fn render(
        &self,
        width: u32,
        height: u32,
    ) -> Result<Arc<Vec<u8>>, &'static str> {
        self.statistics()?;
        raster::render(&self.body, width, height)
    }
    pub fn svg(&self, width: u32, height: u32) -> Result<String, &'static str> {
        self.statistics()?;
        raster::svg(&self.body, width, height)
    }
    pub(crate) fn same_state(&self, other: &Self) -> bool {
        self.layer == other.layer && Arc::ptr_eq(&self.body, &other.body)
    }
}
impl LayerContent {
    pub fn matrix(&self) -> Affine {
        match self {
            Self::Text(t) => t.matrix,
            Self::Vector(v) => v.matrix,
        }
    }
    pub fn transformed(&self, m: Affine) -> Self {
        let mut result = self.clone();
        match &mut result {
            Self::Text(t) => t.matrix = t.matrix.then(m),
            Self::Vector(v) => v.matrix = v.matrix.then(m),
        }
        result
    }
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Text(_) => "텍스트",
            Self::Vector(_) => "벡터",
        }
    }
}
impl Document {
    pub fn native_content(&self, index: usize) -> Option<&ContentNode> {
        let id = self.layers.get(index)?.id;
        self.contents
            .binary_search_by_key(&id.0, |n| n.layer.0)
            .ok()
            .map(|i| &self.contents[i])
    }
    pub fn create_content_layer(
        &mut self,
        body: LayerContent,
        name: String,
    ) -> Result<LayerId, &'static str> {
        if !self.validate()
            || self.layers.len() >= 64
            || !self.can_allocate_layers(1)
        {
            return Err("새 레이어를 만들 수 없습니다.");
        }
        if let Some(reason) = self.layer_insertion_reason() {
            return Err(reason);
        }
        if name.trim().is_empty()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
        {
            return Err("레이어 이름을 확인해 주세요.");
        }
        let node = ContentNode::new(LayerId(self.next_layer_id), body)?;
        let bytes = self
            .logical_pixel_bytes()
            .saturating_add(u64::from(self.width) * u64::from(self.height) * 4)
            .saturating_add(node.logical_bytes());
        if bytes > 512 * 1024 * 1024 {
            return Err("편집 데이터가 512 MiB 논리 예산을 초과합니다.");
        }
        let pixels = node.render(self.width, self.height)?;
        let before = self.clone();
        self.add_layer();
        if self.layers.len() != before.layers.len() + 1 {
            return Err("새 레이어를 만들지 못했습니다.");
        }
        let id = self.layers[self.active].id;
        debug_assert_eq!(node.layer, id);
        self.layers[self.active].name = name;
        self.layers[self.active].pixels = pixels;
        self.contents.push(node);
        self.contents.sort_by_key(|n| n.layer.0);
        self.touch();
        if !self.validate() {
            *self = before;
            return Err("레이어 데이터가 유효하지 않습니다.");
        }
        Ok(id)
    }
    pub fn replace_content(
        &mut self,
        index: usize,
        body: LayerContent,
    ) -> Result<bool, &'static str> {
        if !self.validate() || !self.layer_ancestors_unlocked(index) {
            return Err("대상 레이어와 부모의 잠금을 해제해 주세요.");
        }
        if self.layers[index].alpha_locked {
            return Err("투명 픽셀 잠금을 해제해 주세요.");
        }
        let previous = self
            .native_content(index)
            .ok_or("텍스트나 벡터 레이어를 선택해 주세요.")?;
        if previous.body.as_ref() == &body {
            return Ok(false);
        }
        let node = ContentNode::new(previous.layer, body)?;
        if self
            .logical_pixel_bytes()
            .saturating_sub(previous.logical_bytes())
            .saturating_add(node.logical_bytes())
            > 512 * 1024 * 1024
        {
            return Err("편집 데이터의 논리 예산을 초과합니다.");
        }
        let pixels = node.render(self.width, self.height)?;
        let position = self
            .contents
            .binary_search_by_key(&node.layer.0, |n| n.layer.0)
            .unwrap();
        self.layers[index].pixels = pixels;
        self.contents[position] = node;
        self.touch();
        Ok(true)
    }
    pub fn rasterize_content(&mut self, index: usize) -> bool {
        if !self.validate() || !self.layer_ancestors_unlocked(index) {
            return false;
        }
        let Some(node) = self.native_content(index) else {
            return false;
        };
        let id = node.layer;
        self.contents.retain(|node| node.layer != id);
        self.touch();
        true
    }
    pub(crate) fn prune_contents(&mut self) {
        self.contents.retain(|node| {
            self.layers.iter().any(|layer| {
                layer.id == node.layer && layer.kind == LayerKind::Raster
            })
        });
    }
    pub(crate) fn duplicate_contents(
        &mut self,
        mapping: &[(LayerId, LayerId)],
    ) {
        let mut copies = Vec::new();
        for (old, new) in mapping {
            if let Some(node) = self.contents.iter().find(|n| n.layer == *old) {
                let mut copy = node.clone();
                copy.layer = *new;
                copies.push(copy);
            }
        }
        self.contents.extend(copies);
        self.contents.sort_by_key(|n| n.layer.0);
    }
}

fn bounded_vec<
    'de,
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
    const LIMIT: usize,
>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    struct Values<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de>
        for Values<T, N>
    {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a bounded content sequence")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Vec<T>, A::Error> {
            use serde::de::Error;
            if seq.size_hint().is_some_and(|n| n > N) {
                return Err(A::Error::custom(
                    "Content sequence limit exceeded",
                ));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(seq.size_hint().unwrap_or(0))
                .map_err(A::Error::custom)?;
            while let Some(value) = seq.next_element()? {
                if values.len() >= N {
                    return Err(A::Error::custom(
                        "Content sequence limit exceeded",
                    ));
                }
                values.push(value);
            }
            values.shrink_to_fit();
            Ok(values)
        }
    }
    d.deserialize_seq(Values::<T, LIMIT>(std::marker::PhantomData))
}
pub(crate) fn deserialize_contents<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<ContentNode>, D::Error> {
    bounded_vec::<D, ContentNode, 64>(d)
}
fn deserialize_points<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Arc<Vec<[f32; 2]>>, D::Error> {
    bounded_vec::<D, [f32; 2], MAX_CONTENT_POINTS>(d).map(Arc::new)
}
fn deserialize_paths<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<VectorPath>, D::Error> {
    bounded_vec::<D, VectorPath, MAX_CONTENT_PATHS>(d)
}
fn deserialize_text<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<String, D::Error> {
    struct Text;
    impl<'de> serde::de::Visitor<'de> for Text {
        type Value = String;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded UTF-8 text")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<String, E> {
            if v.len() > 1024 {
                return Err(E::custom("Text exceeds byte limit"));
            }
            Ok(v.into())
        }
    }
    d.deserialize_str(Text)
}
fn deserialize_font<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Arc<Vec<u8>>, D::Error> {
    struct Font;
    impl<'de> serde::de::Visitor<'de> for Font {
        type Value = Arc<Vec<u8>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a font of at most 16 MiB")
        }
        fn visit_bytes<E: serde::de::Error>(
            self,
            v: &[u8],
        ) -> Result<Self::Value, E> {
            if v.len() > MAX_CONTENT_FONT_BYTES {
                return Err(E::custom("Font exceeds limit"));
            }
            Ok(Arc::new(v.to_vec()))
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            use serde::de::Error;
            if seq.size_hint().is_some_and(|n| n > MAX_CONTENT_FONT_BYTES) {
                return Err(A::Error::custom("Font exceeds limit"));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(seq.size_hint().unwrap_or(0))
                .map_err(A::Error::custom)?;
            while let Some(byte) = seq.next_element()? {
                if bytes.len() >= MAX_CONTENT_FONT_BYTES {
                    return Err(A::Error::custom("Font exceeds limit"));
                }
                bytes.push(byte);
            }
            bytes.shrink_to_fit();
            Ok(Arc::new(bytes))
        }
    }
    d.deserialize_bytes(Font)
}
