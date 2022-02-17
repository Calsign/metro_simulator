use cgmath as cg;

use crate::types;

// 70 mph, maximum speed of BART
const MAX_SPEED: f64 = 31.0;
// recommended by https://link.springer.com/article/10.1007/s40864-015-0012-y
const MAX_ACCEL: f64 = 1.5;
// estimate, in practice this varies based on number of passengers
const STATION_TIME: f64 = 30.0;

trait GetVec {
    fn vec(&self, tile_size: f64) -> cg::Vector2<f64>;
}

impl GetVec for types::MetroKey {
    fn vec(&self, tile_size: f64) -> cg::Vector2<f64> {
        match self {
            types::MetroKey::Key(vec) => vec * tile_size,
            types::MetroKey::Stop(vec, _) => vec * tile_size,
        }
    }
}

/**
 * Represents a speed bound in speed-distance space, which looks like
 * a sqrt function diverging in either direction. The function is
 * given as follows:
 *
 * f(d) = {
 *   b,                   d = t
 *   sqrt(b^2 + 2a(d-t)), d > t
 *   sqrt(b^2 - 2a(d-t)), d < t
 * }
 *
 * or, equivalently:
 *
 * f(d) = sqrt(b^2 + |2a(d-t)|)
 */
#[derive(PartialEq, Debug, Clone)]
pub struct SqrtPair {
    pub station: Option<types::Station>,
    /// distance marker of center
    pub t: f64,
    /// speed bound
    pub b: f64,
    /// max acceleration
    pub a: f64,
}

#[derive(Debug, Copy, Clone)]
struct SqrtPairIntersection {
    /// distance marker of intersection
    t: f64,
    /// speed at intersection
    b: f64,
}

impl SqrtPair {
    fn eval(&self, t: f64) -> f64 {
        (self.b.powi(2) + (2.0 * self.a * (self.t - t)).abs()).sqrt()
    }

    fn intersection(&self, other: &Self) -> Option<SqrtPairIntersection> {
        let (l, r) = if self.t <= other.t {
            (self, other)
        } else {
            (other, self)
        };

        let l_comp = 2.0 * l.a * l.t - l.b.powi(2);
        let r_comp = 2.0 * r.a * r.t + r.b.powi(2);
        let t = (l_comp + r_comp) / (2.0 * (l.a + r.a));

        if t >= l.t && t <= r.t {
            Some(SqrtPairIntersection { t, b: self.eval(t) })
        } else {
            None
        }
    }

    fn intersect_bound(&self, bound: f64) -> Option<(f64, f64)> {
        if bound >= self.b {
            let d = (bound.powi(2) - self.b.powi(2)) / (2.0 * self.a);
            Some((self.t - d, self.t + d))
        } else {
            None
        }
    }

    fn integral(&self, t: f64) -> f64 {
        let f = |x: f64| {
            (self.b.powi(2) + 2.0 * self.a * (x - self.t)).powf(3.0 / 2.0) / (3.0 * self.a)
        };
        f(self.t + (t - self.t).abs()) - f(self.t)
    }

    fn average_speed(&self, t: f64) -> f64 {
        self.integral(t) / (t - self.t).abs()
    }
}

impl PartialOrd for SqrtPair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.intersection(other) {
            Some(_) => None,
            None => self.b.partial_cmp(&other.b),
        }
    }
}

/**
 * Convert each metro key into a speed bound, a SqrtPair.
 */
pub fn speed_bounds(keys: &Vec<types::MetroKey>, tile_size: f64) -> Vec<SqrtPair> {
    use cg::Angle;
    use cg::InnerSpace;
    use cg::MetricSpace;
    use itertools::Itertools;

    let mut speed_bounds = Vec::new();
    let mut t = 0.0; // total distance

    // start from rest
    speed_bounds.push(SqrtPair {
        station: None,
        t,
        b: 0.0,
        a: MAX_ACCEL,
    });

    for (prev_key, key, next_key) in keys.iter().tuple_windows() {
        t += key.vec(tile_size).distance(prev_key.vec(tile_size));
        let (top_speed, station) = match key {
            types::MetroKey::Key(_) => {
                // TODO: account for duplicate key vecs (angle will be undefined)
                let angle_diff = (key.vec(tile_size) - prev_key.vec(tile_size))
                    .angle(next_key.vec(tile_size) - key.vec(tile_size));
                // NOTE: approximation
                let top_speed = MAX_SPEED * (1.0 - angle_diff.sin().abs());
                assert!(
                    top_speed > 0.0,
                    "turns must be less than 90 degrees: {:?} => {:?} => {:?}, angle: {:?}",
                    prev_key,
                    key,
                    next_key,
                    angle_diff,
                );
                (top_speed, None)
            }
            // NOTE: come to a full stop at stops
            types::MetroKey::Stop(_, station) => (0.0, Some(station.clone())),
        };

        speed_bounds.push(SqrtPair {
            station,
            t,
            b: top_speed,
            a: MAX_ACCEL,
        });
    }

    // finish at rest
    speed_bounds.push(SqrtPair {
        station: None,
        // TODO: account for the last key here
        t,
        b: 0.0,
        a: MAX_ACCEL,
    });

    speed_bounds
}

/**
 * Assumes input is sorted by t (i.e. horizontally).
 *
 * This is O(n^2), but in most realistic cases it will be O(n).
 */
fn sqrt_pair_minima(input: Vec<SqrtPair>) -> Vec<SqrtPair> {
    let mut minima: Vec<SqrtPair> = Vec::new();
    for sqrt_pair in input {
        match minima.last() {
            Some(last) => match last.partial_cmp(&sqrt_pair) {
                None => minima.push(sqrt_pair),
                Some(std::cmp::Ordering::Less) => continue,
                Some(std::cmp::Ordering::Greater) => {
                    // peel off entries on the stack
                    // this is what makes this O(n^2)
                    while matches!(minima.last(), Some(last) if last > &sqrt_pair) {
                        std::mem::drop(minima.pop())
                    }
                    minima.push(sqrt_pair);
                }
                Some(std::cmp::Ordering::Equal) => {
                    panic!("got duplicate entries: {:?}, {:?}", last, sqrt_pair)
                }
            },
            None => {
                minima.push(sqrt_pair);
            }
        }
    }
    minima
}

#[derive(PartialEq, Debug, Clone)]
pub struct SpeedKey {
    pub station: Option<types::Station>,
    pub t: f64,
    pub v: f64,
}

/**
 * Converts from a sequence of SqrtPairs in speed-distance space to a
 * sequence of SpeedKeys in speed-time space.
 */
fn time_rectify(minimal_speed_bounds: Vec<SqrtPair>) -> Vec<SpeedKey> {
    use itertools::Itertools;

    let mut speed_keys = Vec::new();
    let mut t = 0.0; // total time

    if let Some(first) = minimal_speed_bounds.first() {
        speed_keys.push(SpeedKey {
            station: first.station.clone(),
            t,
            v: first.b,
        });
    }

    for (left, right) in minimal_speed_bounds.iter().tuple_windows() {
        let intersection = left
            .intersection(right)
            .expect("found two consecutive SqrtPairs with no intersection");

        // boarding time at each station
        if let Some(_) = left.station {
            t += STATION_TIME;
        }

        if intersection.b > MAX_SPEED {
            let (_, l_int) = left.intersect_bound(MAX_SPEED).unwrap();
            let (r_int, _) = right.intersect_bound(MAX_SPEED).unwrap();
            t += (l_int - left.t) / left.average_speed(l_int);
            speed_keys.push(SpeedKey {
                station: None,
                t,
                v: MAX_SPEED,
            });
            t += (r_int - l_int) / MAX_SPEED;
            speed_keys.push(SpeedKey {
                station: None,
                t,
                v: MAX_SPEED,
            });
            t += (right.t - r_int) / right.average_speed(r_int);
            speed_keys.push(SpeedKey {
                station: right.station.clone(),
                t,
                v: right.b,
            });
        } else {
            t += (intersection.t - left.t) / left.average_speed(intersection.t);
            speed_keys.push(SpeedKey {
                station: None,
                t,
                v: intersection.b,
            });
            t += (right.t - intersection.t) / right.average_speed(intersection.t);
            speed_keys.push(SpeedKey {
                station: right.station.clone(),
                t,
                v: right.b,
            });
        }
    }

    speed_keys
}

fn parabolic_key(a: f64, b: f64, c: f64, left: f64, right: f64) -> splines::Key<f64, f64> {
    let f = |t: f64| a * t.powi(2) + b * t + c;
    let fp = |t: f64| 2.0 * a * t + b;

    let p1 = f(left);
    let p2 = f(right);
    let c = f(left) + fp(left) * (right - left) / 2.0;

    let c1 = (2.0 / 3.0) * c + (1.0 / 3.0) * p1;
    let c2 = (2.0 / 3.0) * c + (1.0 / 3.0) * p2;

    splines::Key::new(left, p1, splines::Interpolation::StrokeBezier(c1, c2))
}

fn distance_spline(speed_keys: &Vec<SpeedKey>) -> Vec<splines::Key<f64, f64>> {
    use itertools::Itertools;
    use splines::{Interpolation, Key};

    let mut dist_keys = Vec::new();
    let mut d = 0.0; // total distance

    for (left, right) in speed_keys.iter().tuple_windows() {
        if left.v == right.v {
            dist_keys.push(Key::new(left.t, d, Interpolation::Linear));
        } else {
            // slope of speed curve
            let m = (right.v - left.v) / (right.t - left.t);

            // integrate speed curve to get a parabola
            let a = d;
            let b = left.v;
            let c = m / 2.0;

            dist_keys.push(parabolic_key(a, b, c, left.t, right.t));
        }

        let avg_speed = (left.v + right.v) / 2.0;
        d += avg_speed * (right.t - left.t);
    }

    if let Some(last) = speed_keys.last() {
        dist_keys.push(Key::new(last.t, d, Interpolation::Linear));
    }

    dist_keys
}

pub fn speed_keys(keys: &Vec<types::MetroKey>, tile_size: f64) -> Vec<SpeedKey> {
    // convert each key into a speed bound
    let speed_bounds = speed_bounds(keys, tile_size);

    // identify the minima in the speed bound partial order; only these turn into keys in the final speed curve
    let minima = sqrt_pair_minima(speed_bounds);

    // rectify from speed-distance space to distance-time space
    time_rectify(minima)
}

pub fn timetable(speed_keys: &Vec<SpeedKey>) -> Vec<(types::Station, f64)> {
    use itertools::Itertools;

    let mut timetable = Vec::new();
    for key in speed_keys {
        if let Some(station) = &key.station {
            timetable.push((station.clone(), key.t));
        }
    }

    timetable
}

pub fn dist_spline(keys: &Vec<SpeedKey>) -> splines::Spline<f64, f64> {
    let dist_keys = distance_spline(keys);
    splines::Spline::from_vec(dist_keys)
}

#[cfg(test)]
mod sqrt_pair_tests {
    use crate::timing::{sqrt_pair_minima, SqrtPair};
    use float_cmp::assert_approx_eq;

    const F1: SqrtPair = SqrtPair {
        station: None,
        t: 0.0,
        b: 0.0,
        a: 1.0,
    };

    const F2: SqrtPair = SqrtPair {
        station: None,
        t: 1.0,
        b: 0.0,
        a: 1.0,
    };

    const F3: SqrtPair = SqrtPair {
        station: None,
        t: 0.0,
        b: 1.0,
        a: 1.0,
    };

    const F4: SqrtPair = SqrtPair {
        station: None,
        t: 3.5,
        b: 1.0,
        a: 1.0,
    };

    #[test]
    fn eval() {
        assert_approx_eq!(f64, F1.eval(0.0), 0.0);
        assert_approx_eq!(f64, F1.eval(0.5), 1.0);
        assert_approx_eq!(f64, F1.eval(-0.5), 1.0);
        assert_approx_eq!(f64, F1.eval(2.0), 2.0);
        assert_approx_eq!(f64, F1.eval(-2.0), 2.0);

        assert_approx_eq!(f64, F2.eval(1.0), 0.0);
        assert_approx_eq!(f64, F2.eval(0.5), 1.0);
        assert_approx_eq!(f64, F2.eval(1.5), 1.0);

        assert_approx_eq!(f64, F3.eval(0.0), 1.0);
        assert_approx_eq!(f64, F3.eval(1.5), 2.0);
        assert_approx_eq!(f64, F3.eval(-1.5), 2.0);
    }

    #[test]
    fn intersection() {
        assert_approx_eq!(f64, F1.intersection(&F2).unwrap().t, 0.5);
        assert_approx_eq!(f64, F1.intersection(&F2).unwrap().b, 1.0);

        assert_approx_eq!(f64, F2.intersection(&F1).unwrap().t, 0.5);
        assert_approx_eq!(f64, F2.intersection(&F1).unwrap().b, 1.0);

        assert!(F1.intersection(&F3).is_none());

        assert_approx_eq!(f64, F1.intersection(&F4).unwrap().t, 2.0);
        assert_approx_eq!(f64, F1.intersection(&F4).unwrap().b, 2.0);
    }

    #[test]
    fn intersect_bound() {
        assert_approx_eq!(f64, F1.intersect_bound(1.0).unwrap().0, -0.5);
        assert_approx_eq!(f64, F1.intersect_bound(1.0).unwrap().1, 0.5);

        assert_approx_eq!(f64, F1.intersect_bound(2.0).unwrap().0, -2.0);
        assert_approx_eq!(f64, F1.intersect_bound(2.0).unwrap().1, 2.0);

        assert_eq!(F1.intersect_bound(-1.0), None);

        assert_approx_eq!(f64, F2.intersect_bound(1.0).unwrap().0, 0.5);
        assert_approx_eq!(f64, F2.intersect_bound(1.0).unwrap().1, 1.5);
    }

    #[test]
    fn partial_ord() {
        assert!(!(F1 < F2));
        assert!(!(F2 < F1));
        assert!(F1 < F3);
    }

    #[test]
    fn integral() {
        assert_approx_eq!(f64, F1.integral(2.0), 8.0 / 3.0);
        assert_approx_eq!(f64, F2.integral(3.0), 8.0 / 3.0);
        assert_approx_eq!(f64, F3.integral(1.5), 7.0 / 3.0);
        assert_approx_eq!(f64, F4.integral(5.0), 7.0 / 3.0);
    }

    #[test]
    fn minima() {
        assert_eq!(sqrt_pair_minima(vec!()), vec!());
        assert_eq!(sqrt_pair_minima(vec!(F1)), vec!(F1));
        assert_eq!(sqrt_pair_minima(vec!(F1, F2)), vec!(F1, F2));
        assert_eq!(sqrt_pair_minima(vec!(F1, F3)), vec!(F1));
        assert_eq!(sqrt_pair_minima(vec!(F3, F1)), vec!(F1));

        assert_eq!(sqrt_pair_minima(vec!(F1, F3, F2)), vec!(F1, F2));
        assert_eq!(sqrt_pair_minima(vec!(F3, F1, F2)), vec!(F1, F2));

        assert_eq!(sqrt_pair_minima(vec!(F1, F2, F4)), vec!(F1, F2, F4));
    }
}
