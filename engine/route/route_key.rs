use serde::{Deserialize, Serialize};

use crate::common::Mode;

#[derive(Debug, Copy, Clone, derive_more::Constructor, Serialize, Deserialize)]
pub struct RouteKey {
    pub position: (f32, f32),
    pub dist: f32,
    pub time: f32,
    pub mode: Mode,
}

impl splines::Interpolate<f32> for RouteKey {
    fn step(t: f32, threshold: f32, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn lerp(t: f32, a: Self, b: Self) -> Self {
        Self {
            position: (
                f32::lerp(t, a.position.0, b.position.0),
                f32::lerp(t, a.position.1, b.position.1),
            ),
            dist: f32::lerp(t, a.dist, b.dist),
            time: f32::lerp(t, a.time, b.time),
            mode: a.mode,
        }
    }

    fn cosine(t: f32, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_hermite(
        t: f32,
        x: (f32, Self),
        a: (f32, Self),
        b: (f32, Self),
        y: (f32, Self),
    ) -> Self {
        unimplemented!()
    }

    fn quadratic_bezier(t: f32, a: Self, u: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier_mirrored(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }
}
