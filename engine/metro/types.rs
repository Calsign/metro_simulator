use cgmath as cg;
use serde::{Deserialize, Serialize};

use crate::color;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub address: quadtree::Address,
}

/** Used only in constructing a MetroLine. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetroKey {
    Key(cg::Vector2<f64>),
    // NOTE: u64 because stations have to be on discrete unit tiles
    Stop(cg::Vector2<f64>, Station),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroLine {
    pub id: u64,
    pub color: color::Color,
    pub name: String,
    keys: Vec<MetroKey>,
    spline: splines::Spline<f64, cg::Vector2<f64>>,
    length: f64,
    stops: Vec<Station>,
}

impl PartialEq for MetroLine {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MetroLine {}

impl Ord for MetroLine {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for MetroLine {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl MetroLine {
    pub fn new(id: u64, color: color::Color, name: String) -> Self {
        Self {
            id,
            color,
            name,
            keys: vec![],
            spline: splines::Spline::from_vec(vec![]),
            length: 0.0,
            stops: vec![],
        }
    }

    pub fn get_keys(&self) -> &Vec<MetroKey> {
        &self.keys
    }

    pub fn set_keys(&mut self, keys: Vec<MetroKey>) {
        use cg::MetricSpace;

        let mut spline_keys: Vec<splines::Key<f64, cg::Vector2<f64>>> = Vec::new();
        let mut stops = Vec::new();
        let mut t = 0.0;
        for key in &keys {
            let vec = match key {
                MetroKey::Key(vec) => vec,
                MetroKey::Stop(vec, station) => {
                    stops.push(station.clone());
                    vec
                }
            };
            if let Some(last) = spline_keys.last() {
                t += vec.distance(last.value);
            }
            spline_keys.push(splines::Key::new(t, *vec, splines::Interpolation::Linear));
        }

        self.keys = keys;
        self.spline = splines::Spline::from_vec(spline_keys);
        self.length = t;
        self.stops = stops;
    }

    pub fn length(&self) -> f64 {
        self.length
    }

    pub fn stops(&self) -> &Vec<Station> {
        &self.stops
    }

    pub fn visit_spline<V, E>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
    ) -> Result<(), E>
    where
        V: SplineVisitor<E>,
    {
        if self.spline.len() == 0 {
            return Ok(());
        }

        let (min_x, max_x, min_y, max_y) = (
            rect.min_x as f64,
            rect.max_x as f64,
            rect.min_y as f64,
            rect.max_y as f64,
        );

        let cx = (max_x + min_x) / 2.0;
        let cy = (max_y + min_y) / 2.0;
        let rx = (max_x - min_x) / 2.0;
        let ry = (max_y - min_y) / 2.0;

        let total = (self.length / step).ceil() as u64;
        let mut i = 0;
        while i <= total {
            // probe for points in the rectangle
            let (point, t) = loop {
                let t = (i as f64) * step;
                let point = self.spline.clamped_sample(t).unwrap();
                // compute Manhatten distance between point and rectangle
                let dist = f64::min(f64::abs(point.x - cx) - rx, f64::abs(point.y - cy) - ry);
                if dist <= step || i > total {
                    i += 1;
                    break (point, t);
                } else {
                    i += f64::max(f64::floor(dist / step), 1.0) as u64;
                }
            };

            visitor.visit(self, point, t)?;
        }
        Ok(())
    }

    pub fn visit_keys<V, E>(&self, visitor: &mut V, rect: &quadtree::Rect) -> Result<(), E>
    where
        V: KeyVisitor<E>,
    {
        if self.keys.len() == 0 {
            return Ok(());
        }

        for key in &self.keys {
            let loc = match key {
                MetroKey::Key(loc) => loc,
                MetroKey::Stop(loc, _) => loc,
            };

            if loc.x >= rect.min_x as f64
                && loc.x <= rect.max_x as f64
                && loc.y >= rect.min_y as f64
                && loc.y <= rect.max_y as f64
            {
                visitor.visit(self, key)?;
            }
        }

        Ok(())
    }
}

pub trait SplineVisitor<E> {
    fn visit(&mut self, line: &MetroLine, vertex: cg::Vector2<f64>, t: f64) -> Result<(), E>;
}

pub trait KeyVisitor<E> {
    fn visit(&mut self, line: &MetroLine, key: &MetroKey) -> Result<(), E>;
}
