use crate::network::Key;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TimingConfig {
    pub tile_size: f64,
    pub max_speed: f64,
    pub max_acceleration: f64,
    pub start_speed: f64,
    pub end_speed: f64,
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

    fn travel_time(&self, t: f64) -> f64 {
        (t - self.t).abs() / ((self.eval(t) + self.b) / 2.0)
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
 * Convert each key into a speed bound, a SqrtPair.
 */
pub fn speed_bounds(keys: &[Key], config: &TimingConfig) -> Vec<SqrtPair> {
    use cgmath::Angle;
    use cgmath::InnerSpace;
    use cgmath::MetricSpace;
    use itertools::Itertools;

    let mut speed_bounds = Vec::new();
    let mut t = 0.0; // total distance

    speed_bounds.push(SqrtPair {
        t,
        b: config.start_speed,
        a: config.max_acceleration,
    });

    for (prev_key, key, next_key) in keys.iter().tuple_windows() {
        t += key.distance(*prev_key) * config.tile_size;

        // TODO: account for duplicate key vecs (angle will be undefined)
        let angle_diff = (key - prev_key).angle(next_key - key);
        // NOTE: approximation
        let top_speed = config.max_speed * (1.0 - angle_diff.sin().abs());
        assert!(
            top_speed > 0.0,
            "turns must be less than 90 degrees: {:?} => {:?} => {:?}, angle: {:?}",
            prev_key,
            key,
            next_key,
            angle_diff,
        );

        speed_bounds.push(SqrtPair {
            t,
            b: top_speed,
            a: config.max_acceleration,
        });
    }

    if keys.len() >= 2 {
        // account for the distance of the last key
        let last = &keys[keys.len() - 1];
        let second_to_last = &keys[keys.len() - 2];
        t += last.distance(*second_to_last) * config.tile_size;
    }

    speed_bounds.push(SqrtPair {
        t,
        b: config.end_speed,
        a: config.max_acceleration,
    });

    speed_bounds
}

/**
 * Assumes input is sorted by t (i.e. horizontally).
 *
 * This is O(n) because each entry can be added and removed at most once.
 */
fn sqrt_pair_minima(input: Vec<SqrtPair>) -> Vec<SqrtPair> {
    let mut minima: Vec<SqrtPair> = Vec::new();
    for sqrt_pair in input {
        match minima.last() {
            Some(last) => match last.partial_cmp(&sqrt_pair) {
                None => minima.push(sqrt_pair),
                Some(std::cmp::Ordering::Less) => continue,
                Some(std::cmp::Ordering::Greater) => {
                    // Peel off entries on the stack. This looks like it could make this O(n^2), but
                    // we will only ever pull each entry off once.
                    while matches!(minima.last(), Some(last) if last > &sqrt_pair) {
                        std::mem::drop(minima.pop())
                    }
                    minima.push(sqrt_pair);
                }
                Some(std::cmp::Ordering::Equal) => (),
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
    pub t: f64,
    pub v: f64,
}

/**
 * Converts from a sequence of SqrtPairs in speed-distance space to a
 * sequence of SpeedKeys in speed-time space.
 */
fn time_rectify(minimal_speed_bounds: Vec<SqrtPair>, config: &TimingConfig) -> Vec<SpeedKey> {
    use itertools::Itertools;

    let mut speed_keys = Vec::new();
    let mut t = 0.0; // total time

    if let Some(first) = minimal_speed_bounds.first() {
        speed_keys.push(SpeedKey { t, v: first.b });
    }

    for (left, right) in minimal_speed_bounds.iter().tuple_windows() {
        let intersection = left
            .intersection(right)
            .expect("found two consecutive SqrtPairs with no intersection");

        if intersection.b > config.max_speed {
            let (_, l_int) = left.intersect_bound(config.max_speed).unwrap();
            let (r_int, _) = right.intersect_bound(config.max_speed).unwrap();
            t += left.travel_time(l_int);
            speed_keys.push(SpeedKey {
                t,
                v: config.max_speed,
            });
            t += (r_int - l_int) / config.max_speed;
            speed_keys.push(SpeedKey {
                t,
                v: config.max_speed,
            });
            t += right.travel_time(r_int);
            speed_keys.push(SpeedKey { t, v: right.b });
        } else {
            if left.t != intersection.t {
                t += left.travel_time(intersection.t)
            }
            speed_keys.push(SpeedKey {
                t,
                v: intersection.b,
            });
            if right.t != intersection.t {
                t += right.travel_time(intersection.t)
            }
            speed_keys.push(SpeedKey { t, v: right.b });
        }
    }

    speed_keys
}

fn _parabolic_key(a: f64, b: f64, c: f64, left: f64, right: f64) -> splines::Key<f64, f64> {
    let f = |t: f64| a * t.powi(2) + b * t + c;
    let fp = |t: f64| 2.0 * a * t + b;

    let p1 = f(left);
    let p2 = f(right);
    let c = f(left) + fp(left) * (right - left) / 2.0;

    let c1 = (2.0 / 3.0) * c + (1.0 / 3.0) * p1;
    let c2 = (2.0 / 3.0) * c + (1.0 / 3.0) * p2;

    splines::Key::new(left, p1, splines::Interpolation::StrokeBezier(c1, c2))
}

fn distance_spline(speed_keys: &[SpeedKey]) -> Vec<splines::Key<f64, f64>> {
    use itertools::Itertools;
    use splines::{Interpolation, Key};

    let mut dist_keys = Vec::new();
    let mut d = 0.0; // total distance

    for (left, right) in speed_keys.iter().tuple_windows() {
        assert!(right.t >= left.t, "{:?}, {:?}", left, right);
        if left.v == right.v {
            dist_keys.push(Key::new(left.t, d, Interpolation::Linear));
        } else {
            // slope of speed curve
            let m = (right.v - left.v) / (right.t - left.t);

            // integrate speed curve to get a parabola
            // TODO: not currently working :(
            let _a = m / 2.0;
            let _b = left.v - m * left.t;
            let _c = left.t - left.t * left.v + 0.5 * m * left.t.powi(2) + m * left.t;

            // dist_keys.push(parabolic_key(a, b, c, left.t, right.t));
            dist_keys.push(Key::new(left.t, d, Interpolation::Linear));
        }

        let avg_speed = (left.v + right.v) / 2.0;
        d += avg_speed * (right.t - left.t);
    }

    if let Some(last) = speed_keys.last() {
        dist_keys.push(Key::new(last.t, d, Interpolation::Linear));
    }

    dist_keys
}

pub fn speed_keys(keys: &[Key], config: &TimingConfig) -> Vec<SpeedKey> {
    // convert each key into a speed bound
    let speed_bounds = speed_bounds(keys, config);

    // identify the minima in the speed bound partial order; only these turn into keys in the final speed curve
    let minima = sqrt_pair_minima(speed_bounds);

    // rectify from speed-distance space to distance-time space
    time_rectify(minima, config)
}

pub fn dist_spline(keys: &[SpeedKey]) -> splines::Spline<f64, f64> {
    let dist_keys = distance_spline(keys);
    splines::Spline::from_vec(dist_keys)
}

#[cfg(test)]
mod sqrt_pair_tests {
    use crate::timing::{sqrt_pair_minima, SqrtPair};
    use float_cmp::assert_approx_eq;

    const F1: SqrtPair = SqrtPair {
        t: 0.0,
        b: 0.0,
        a: 1.0,
    };

    const F2: SqrtPair = SqrtPair {
        t: 1.0,
        b: 0.0,
        a: 1.0,
    };

    const F3: SqrtPair = SqrtPair {
        t: 0.0,
        b: 1.0,
        a: 1.0,
    };

    const F4: SqrtPair = SqrtPair {
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
        assert_eq!(F1.partial_cmp(&F2), None);
        assert!(F1 < F3);
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

#[cfg(test)]
mod dist_spline_tests {
    use crate::timing::{distance_spline, time_rectify, SpeedKey, SqrtPair, TimingConfig};
    use float_cmp::assert_approx_eq;

    #[test]
    fn time_rectify_test() {
        let speed_keys = time_rectify(
            vec![
                SqrtPair {
                    t: 0.0,
                    b: 0.0,
                    a: 0.5,
                },
                SqrtPair {
                    t: 2.0,
                    b: 0.0,
                    a: 0.5,
                },
            ],
            &TimingConfig {
                tile_size: 1.0,
                max_speed: 1.0,
                max_acceleration: 1.0,
                start_speed: 0.0,
                end_speed: 0.0,
            },
        );
        assert_eq!(speed_keys.len(), 3);
        assert_approx_eq!(f64, speed_keys[0].t, 0.0);
        assert_approx_eq!(f64, speed_keys[0].v, 0.0);
        assert_approx_eq!(f64, speed_keys[1].t, 2.0);
        assert_approx_eq!(f64, speed_keys[1].v, 1.0);
        assert_approx_eq!(f64, speed_keys[2].t, 4.0);
        assert_approx_eq!(f64, speed_keys[2].v, 0.0);
    }

    #[test]
    fn distance_spline_test() {
        let keys = distance_spline(&[
            SpeedKey { t: 0.0, v: 0.0 },
            SpeedKey { t: 2.0, v: 1.0 },
            SpeedKey { t: 4.0, v: 0.0 },
        ]);
        assert_eq!(keys.len(), 3);
        assert_approx_eq!(f64, keys[0].t, 0.0);
        assert_approx_eq!(f64, keys[0].value, 0.0);
        assert_approx_eq!(f64, keys[1].t, 2.0);
        assert_approx_eq!(f64, keys[1].value, 1.0);
        assert_approx_eq!(f64, keys[2].t, 4.0);
        assert_approx_eq!(f64, keys[2].value, 2.0);
    }
}
