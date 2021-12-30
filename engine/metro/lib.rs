use cgmath as cg;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl From<Color> for (u8, u8, u8) {
    fn from(color: Color) -> (u8, u8, u8) {
        (color.red, color.green, color.blue)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from(color: (u8, u8, u8)) -> Self {
        let (red, green, blue) = color;
        Color { red, green, blue }
    }
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
    pub length: f64,
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
            length: t,
            stations,
        }
    }

    pub fn visit_spline<E>(&self, visitor: &mut dyn SplineVisitor<E>, step: f64) -> Result<(), E> {
        if self.spline.len() == 0 {
            return Ok(());
        }
        let total = (self.length / step).ceil() as u64;
        for i in 0..=total {
            let t = (i as f64) * step;
            visitor.visit(self, self.spline.clamped_sample(t).unwrap(), t)?;
        }
        Ok(())
    }
}

pub trait SplineVisitor<E> {
    fn visit(&mut self, line: &MetroLine, vertex: cg::Vector2<f64>, t: f64) -> Result<(), E>;
}
