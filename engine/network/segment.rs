use serde::{Deserialize, Serialize};

use crate::junction::JunctionHandle;
use crate::network::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SegmentHandle(pub(crate) u64);

impl SegmentHandle {
    pub fn inner(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment<T> {
    pub id: SegmentHandle,
    pub data: T,
    keys: Vec<Key>,
    pub bounds: quadtree::Rect,
    spline: splines::Spline<f64, Key>,
    length: f64,
    start: JunctionHandle,
    end: JunctionHandle,
}

id_cmp::id_cmp!(Segment<T>, id, T);

impl<T> Segment<T> {
    pub(crate) fn new(
        id: SegmentHandle,
        data: T,
        start_junction: JunctionHandle,
        end_junction: JunctionHandle,
    ) -> Self {
        Self {
            id,
            data,
            keys: Vec::new(),
            bounds: quadtree::Rect::xywh(0, 0, 0, 0),
            spline: splines::Spline::from_vec(Vec::new()),
            length: 0.0,
            start: start_junction,
            end: end_junction,
        }
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn spline_keys(&self) -> &[splines::Key<f64, Key>] {
        self.spline.keys()
    }

    pub fn set_keys(&mut self, keys: Vec<Key>) {
        use cgmath::MetricSpace;

        let mut spline_keys: Vec<splines::Key<f64, Key>> = Vec::new();
        let mut t = 0.0;
        for key in &keys {
            if let Some(last) = spline_keys.last() {
                t += key.distance(last.value);
            }
            spline_keys.push(splines::Key::new(t, *key, splines::Interpolation::Linear));
        }

        self.bounds = spline_util::compute_bounds(&keys, |key| (key.x, key.y));
        self.keys = keys;
        self.spline = splines::Spline::from_vec(spline_keys);
        self.length = t;
    }

    pub fn length(&self) -> f64 {
        self.length
    }

    pub fn start_junction(&self) -> JunctionHandle {
        self.start
    }

    pub fn end_junction(&self) -> JunctionHandle {
        self.end
    }

    pub fn spline(&self) -> &splines::Spline<f64, Key> {
        &self.spline
    }

    pub fn visit_spline<V, E>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
    ) -> Result<(), E>
    where
        V: spline_util::SplineVisitor<Self, Key, E>,
    {
        spline_util::visit_spline(
            self,
            &self.spline,
            self.length,
            visitor,
            step,
            rect,
            |pos| pos,
        )
    }

    pub fn visit_keys<V, E>(&self, visitor: &mut V, rect: &quadtree::Rect) -> Result<(), E>
    where
        V: KeyVisitor<T, E>,
    {
        if self.keys.is_empty() {
            return Ok(());
        }

        for key in &self.keys {
            if key.x >= rect.min_x as f64
                && key.x <= rect.max_x as f64
                && key.y >= rect.min_y as f64
                && key.y <= rect.max_y as f64
            {
                visitor.visit(self, key)?;
            }
        }

        Ok(())
    }
}

pub trait KeyVisitor<T, E> {
    fn visit(&mut self, segment: &Segment<T>, key: &Key) -> Result<(), E>;
}
