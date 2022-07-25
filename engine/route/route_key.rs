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
    fn step(_t: f32, _threshold: f32, _a: Self, _b: Self) -> Self {
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

    fn cosine(_t: f32, _a: Self, _b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_hermite(
        _t: f32,
        _x: (f32, Self),
        _a: (f32, Self),
        _b: (f32, Self),
        _y: (f32, Self),
    ) -> Self {
        unimplemented!()
    }

    fn quadratic_bezier(_t: f32, _a: Self, _u: Self, _b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier(_t: f32, _a: Self, _u: Self, _v: Self, _b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier_mirrored(_t: f32, _a: Self, _u: Self, _v: Self, _b: Self) -> Self {
        unimplemented!()
    }
}
