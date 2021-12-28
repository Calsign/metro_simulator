use cgmath as cg;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub address: quadtree::Address,
}

/** Used only in constructing a MetroLine. */
pub enum MetroKey {
    Key(cg::Vector2<f64>),
    // NOTE: u64 because stations have to be on discrete unit tiles
    Station(cg::Vector2<u64>, Station),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroLine {
    pub id: u64,
    pub color: Color,
    pub spline: splines::Spline<f64, cg::Vector2<f64>>,
    pub stations: Vec<Station>,
}

impl MetroLine {
    pub fn new(id: u64, color: Color, keys: Vec<MetroKey>) -> Self {
        use cg::MetricSpace;

        let mut spline_keys: Vec<splines::Key<f64, cg::Vector2<f64>>> = Vec::new();
        let mut stations = Vec::new();
        let mut t = 0.0;
        for key in keys {
            let vec = match key {
                MetroKey::Key(vec) => vec,
                MetroKey::Station(vec, station) => {
                    stations.push(station);
                    vec.cast().unwrap()
                }
            };
            if let Some(last) = spline_keys.last() {
                t += vec.distance(last.value);
            }
            spline_keys.push(splines::Key::new(t, vec, splines::Interpolation::Linear));
        }

        MetroLine {
            id,
            color,
            spline: splines::Spline::from_vec(spline_keys),
            stations,
        }
    }
}
