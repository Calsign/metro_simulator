use std::collections::HashMap;

use cgmath as cg;
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

pub use spline_util::SplineVisitor;

use crate::color;
use crate::schedule::Schedule;

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

#[derive(Debug, Clone)]
pub struct Splines {
    /// spline mapping distance (in coordinate space) along spline to location on map
    pub spline: splines::Spline<f64, cg::Vector2<f64>>,
    /// total length of spline (in coordinate space)
    pub length: f64,
    /// stops along the metro line
    pub stops: Vec<Station>,
    /// spline mapping time to distance (in meters) along spline
    pub dist_spline: splines::Spline<f64, f64>,
    /// list of (stop, time) tuples
    pub timetable: Vec<(Station, f64)>,
    /// mapping from station to distance (in coordinate space)
    pub dist_map: HashMap<quadtree::Address, f64>,
    /// mapping from station address to time
    pub time_map: HashMap<quadtree::Address, f64>,
    /// mapping from station address to dist_spline key index
    pub index_map: HashMap<quadtree::Address, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroLine {
    pub id: u64,
    pub color: color::Color,
    pub speed_limit: u32,
    pub name: String,
    pub schedule: Schedule,
    tile_size: f64,
    /// the MetroKeys defining the metro line
    keys: Vec<MetroKey>,
    pub bounds: quadtree::Rect,
    /// this is fully determined by the keys
    #[serde(skip)]
    splines: OnceCell<Splines>,
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
    pub fn new(
        id: u64,
        color: color::Color,
        speed_limit: u32,
        name: String,
        tile_size: f64,
    ) -> Self {
        Self {
            id,
            color,
            speed_limit,
            name,
            // TODO: generate metro schedules instead of hard-coding them like this
            schedule: Schedule::fixed_frequency(60 * 15),
            tile_size,
            keys: Vec::new(),
            bounds: quadtree::Rect::xywh(0, 0, 0, 0),
            splines: OnceCell::new(),
        }
    }

    pub fn get_keys(&self) -> &Vec<MetroKey> {
        &self.keys
    }

    pub fn set_keys(&mut self, keys: Vec<MetroKey>) {
        self.bounds = spline_util::compute_bounds(&keys, |key| match key {
            MetroKey::Key(vec) | MetroKey::Stop(vec, _) => (vec.x, vec.y),
        });
        self.keys = keys;
    }

    fn construct_splines(&self) -> Splines {
        use cg::MetricSpace;

        let mut spline_keys: Vec<splines::Key<f64, cg::Vector2<f64>>> = Vec::new();
        let mut stops = Vec::new();
        let mut dist_map = HashMap::new();
        let mut t = 0.0;
        for key in &self.keys {
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
            if let MetroKey::Stop(_, station) = key {
                dist_map.insert(station.address, t);
            }
        }

        let speed_keys =
            crate::timing::speed_keys(&self.keys, self.tile_size, self.speed_limit as f64);
        let dist_spline = crate::timing::dist_spline(&speed_keys);
        let timetable = crate::timing::timetable(&speed_keys);

        let mut time_map = HashMap::new();
        for (station, time) in &timetable {
            time_map.insert(station.address, *time);
        }

        assert_eq!(speed_keys.len(), dist_spline.len());

        let mut index_map = HashMap::new();
        for (i, speed_key) in speed_keys.iter().enumerate() {
            if let Some(station) = &speed_key.station {
                index_map.insert(station.address, i);
            }
        }

        Splines {
            spline: splines::Spline::from_vec(spline_keys),
            length: t,
            stops,
            dist_spline,
            timetable,
            dist_map,
            time_map,
            index_map,
        }
    }

    pub fn get_splines(&self) -> &Splines {
        self.splines.get_or_init(|| self.construct_splines())
    }

    pub fn visit_spline<V, E>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
    ) -> Result<(), E>
    where
        V: SplineVisitor<Self, cgmath::Vector2<f64>, E>,
    {
        spline_util::visit_spline(
            self,
            &self.get_splines().spline,
            self.get_splines().length,
            visitor,
            step,
            rect,
            |pos| pos,
        )
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

pub trait KeyVisitor<E> {
    fn visit(&mut self, line: &MetroLine, key: &MetroKey) -> Result<(), E>;
}
