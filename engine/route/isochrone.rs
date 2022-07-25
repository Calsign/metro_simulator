use std::collections::HashMap;

use crate::base_graph::Graph;
use crate::common::{Error, Mode};

pub struct Isochrone {
    pub travel_times: HashMap<(u64, u64), f64>,
    pub focus: quadtree::Address,
    pub mode: Mode,
}

pub fn calculate_isochrone(
    mut base_graph: std::cell::RefMut<Graph>,
    focus: quadtree::Address,
    mode: Mode,
) -> Result<Isochrone, Error> {
    // TODO: This is a very naive implementation, simply calculating the shortest path for each
    // possible destination. Fortunately the networks here are very simple, so this is tractable.
    // A superior implementation would require modifying fast_paths.
    // See https://github.com/easbar/fast_paths/issues/5.

    let mut isochrone = Isochrone {
        travel_times: HashMap::new(),
        focus,
        mode,
    };

    let (start_x, start_y) = focus.to_xy_f64();
    let nearest = match base_graph.terminal_nodes[mode].find_nearest(start_x, start_y) {
        Some(nearest) => nearest,
        None => return Err(Error::NoTerminalNodeFound(focus)),
    };

    // TODO: using this clone to avoid borrowing issues, but it shouldn't be necessary
    for entry in base_graph.terminal_nodes[mode].entries().clone() {
        // f64 is unhashable
        let (x, y) = (entry.x as u64, entry.y as u64);
        let travel_time = match base_graph.graph.query(nearest, entry.data) {
            Some(shortest_path) => shortest_path.get_weight() as f64,
            None => f64::INFINITY,
        };
        isochrone.travel_times.insert((x, y), travel_time);
    }

    Ok(isochrone)
}

pub struct IsochroneMap {
    pub isochrone: Isochrone,
    map: imageproc::definitions::Image<image::Luma<f64>>,
    downsample: u64,
    scale_factor: f64,
}

impl IsochroneMap {
    pub fn get_travel_time_sq(&self, x: u64, y: u64) -> f64 {
        let (x, y) = ((x / self.downsample) as u32, (y / self.downsample) as u32);
        let pixel = self.map.get_pixel_checked(x, y);
        let unwrapped = pixel.unwrap_or_else(|| panic!("coords out of bounds: {}, {}", x, y));
        // need to square the scale factor as well
        unwrapped.0[0] / self.scale_factor.powi(2)
    }

    pub fn get_travel_time(&self, x: u64, y: u64) -> f64 {
        // Re-computing sqrt for each access is slow. Currently this is just used for drawing. If we
        // do use isochrones in a hot loop in the future, I suspect that we will only be sampling
        // individual positions, such that pre-calculating all values will not be worthwhile.
        self.get_travel_time_sq(x, y).sqrt()
    }
}

pub fn calculate_isochrone_map(
    isochrone: Isochrone,
    config: &state::Config,
    block_size: f32,
) -> Result<IsochroneMap, Error> {
    // round to power of two
    let downsample = config.even_downsample(block_size) as u64;
    let dim = config.tile_width() / downsample as u32;

    // we need to scale up so that each pixel corresponds to one second
    let meters_per_pixel = config.min_tile_size as u64 * downsample;
    let scale_factor = isochrone.mode.linear_speed() / meters_per_pixel as f64;

    let mut raw_map: imageproc::definitions::Image<image::Luma<f64>> =
        image::ImageBuffer::new(dim, dim);
    raw_map.fill(f64::INFINITY);

    for ((x, y), travel_time) in &isochrone.travel_times {
        // the imageproc implementation does not handle squaring
        // we need to square since this is a squared distance transform
        let val = (travel_time * scale_factor).powi(2);
        raw_map.put_pixel(
            (*x / downsample) as u32,
            (*y / downsample) as u32,
            image::Luma([val]),
        );
    }

    // NOTE: Using patched imageproc with an implementation of a weighted distance transform.
    // See patches/imageproc__weighted_distance_transform.patch.
    let map = imageproc::distance_transform::euclidean_squared_distance_transform(&raw_map);

    Ok(IsochroneMap {
        isochrone,
        map,
        downsample,
        scale_factor,
    })
}
