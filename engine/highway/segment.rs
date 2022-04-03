use cgmath as cg;
use serde::{Deserialize, Serialize};

pub use spline_util::SplineVisitor;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct HighwayData {
    pub name: Option<String>,
    pub refs: Vec<String>,
    pub lanes: Option<u32>,
    pub speed_limit: Option<u32>,
}

impl HighwayData {
    pub fn new(
        name: Option<String>,
        refs: Vec<String>,
        lanes: Option<u32>,
        speed_limit: Option<u32>,
    ) -> Self {
        Self {
            name,
            refs,
            lanes,
            speed_limit,
        }
    }
}

pub type HighwayKey = cg::Vector2<f64>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighwaySegment {
    pub id: u64,
    pub data: HighwayData,
    keys: Vec<HighwayKey>,
    spline: splines::Spline<f64, HighwayKey>,
    length: f64,
    start_junction: u64,
    end_junction: u64,
}

impl PartialEq for HighwaySegment {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for HighwaySegment {}

impl Ord for HighwaySegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for HighwaySegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl HighwaySegment {
    pub(crate) fn new(id: u64, data: HighwayData, start_junction: u64, end_junction: u64) -> Self {
        Self {
            id,
            data,
            keys: vec![],
            spline: splines::Spline::from_vec(vec![]),
            length: 0.0,
            start_junction,
            end_junction,
        }
    }

    pub fn get_keys(&self) -> &Vec<HighwayKey> {
        &self.keys
    }

    pub fn set_keys(&mut self, keys: Vec<HighwayKey>) {
        use cg::MetricSpace;

        let mut spline_keys: Vec<splines::Key<f64, HighwayKey>> = Vec::new();
        let mut t = 0.0;
        for key in &keys {
            if let Some(last) = spline_keys.last() {
                t += key.distance(last.value);
            }
            spline_keys.push(splines::Key::new(t, *key, splines::Interpolation::Linear));
        }

        self.keys = keys;
        self.spline = splines::Spline::from_vec(spline_keys);
        self.length = t;
    }

    pub fn length(&self) -> f64 {
        self.length
    }

    pub fn start_junction(&self) -> u64 {
        self.start_junction
    }

    pub fn end_junction(&self) -> u64 {
        self.end_junction
    }

    pub fn visit_spline<V, E>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
    ) -> Result<(), E>
    where
        V: SplineVisitor<Self, E>,
    {
        spline_util::visit_spline(self, &self.spline, self.length, visitor, step, rect)
    }

    pub fn visit_keys<V, E>(&self, visitor: &mut V, rect: &quadtree::Rect) -> Result<(), E>
    where
        V: KeyVisitor<E>,
    {
        if self.keys.len() == 0 {
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

pub trait KeyVisitor<E> {
    fn visit(&mut self, segment: &HighwaySegment, key: &HighwayKey) -> Result<(), E>;
}
