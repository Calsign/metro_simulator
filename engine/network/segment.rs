use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::junction::JunctionHandle;
use crate::network::Key;
use crate::timing::TimingConfig;

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
    /// spline mapping distance (in coordinate space) along spline to location on map
    spline: splines::Spline<f64, Key>,
    /// length (in coordinate space)
    length: f64,
    /// spline mapping time to distance (in meters) along spline
    #[serde(skip)]
    dist_spline: OnceCell<(TimingConfig, splines::Spline<f64, f64>)>,
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
            dist_spline: OnceCell::new(),
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

    fn construct_dist_spline(&self, config: &TimingConfig) -> splines::Spline<f64, f64> {
        let speed_keys = crate::timing::speed_keys(&self.keys, config);
        crate::timing::dist_spline(&speed_keys)
    }

    pub fn dist_spline(&self, config: TimingConfig) -> &splines::Spline<f64, f64> {
        // TODO: if we're really serious about having multiple configs, we can make this a memoized
        // function
        let (stored_config, stored_splines) = self
            .dist_spline
            .get_or_init(|| (config, self.construct_dist_spline(&config)));
        assert_eq!(stored_config, &config);
        stored_splines
    }

    pub fn travel_time(&self, config: TimingConfig) -> f64 {
        self.dist_spline(config)
            .keys()
            .last()
            .map(|key| key.t)
            .unwrap_or(0.0)
    }
}

pub trait KeyVisitor<T, E> {
    fn visit(&mut self, segment: &Segment<T>, key: &Key) -> Result<(), E>;
}
