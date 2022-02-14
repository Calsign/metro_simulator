use cgmath as cg;

use crate::types;

// 70 mph, maximum speed of BART
const MAX_SPEED: f64 = 31.0;
// recommended by https://link.springer.com/article/10.1007/s40864-015-0012-y
const MAX_ACCEL: f64 = 1.5;

trait GetVec {
    fn vec(&self) -> &cg::Vector2<f64>;
}

impl GetVec for types::MetroKey {
    fn vec(&self) -> &cg::Vector2<f64> {
        match self {
            types::MetroKey::Key(vec) => vec,
            types::MetroKey::Stop(vec, _) => vec,
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
#[derive(PartialEq, Debug, Copy, Clone)]
struct SqrtPair {
    /// distance marker of center
    t: f64,
    /// speed bound
    b: f64,
    /// max acceleration
    a: f64,
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
fn speed_bounds(keys: &Vec<types::MetroKey>) -> Vec<SqrtPair> {
    use cg::Angle;
    use cg::InnerSpace;
    use cg::MetricSpace;
    use itertools::Itertools;

    let mut speed_bounds = Vec::new();
    let mut t = 0.0; // total distance

    // start from rest
    speed_bounds.push(SqrtPair {
        t,
        b: 0.0,
        a: MAX_ACCEL,
    });

    for (prev_key, key, next_key) in keys.iter().tuples() {
        t += key.vec().distance(*prev_key.vec());
        let top_speed = match key {
            types::MetroKey::Key(_) => {
                // TODO: account for duplicate key vecs (angle will be undefined)
                let angle_diff = (key.vec() - prev_key.vec()).angle(next_key.vec() - key.vec());
                // NOTE: approximation
                let top_speed = MAX_SPEED / angle_diff.cos();
                assert!(top_speed > 0.0, "turns must be less than 90 degrees");
                top_speed
            }
            // NOTE: come to a full stop at stops
            types::MetroKey::Stop(_, _) => 0.0,
        };

        speed_bounds.push(SqrtPair {
            t,
            b: top_speed,
            a: MAX_ACCEL,
        });
    }

    // finish at rest
    speed_bounds.push(SqrtPair {
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

fn time_rectify(minimal_speed_bounds: Vec<SqrtPair>) -> Vec<splines::Key<f64, f64>> {
    let mut time_keys = Vec::new();
    let mut t = 0.0; // total time
    for bound in minimal_speed_bounds {
        // todo
    }

    time_keys
}

pub fn time_plot(keys: &Vec<types::MetroKey>) -> splines::Spline<f64, f64> {
    // convert each key into a speed bound
    let speed_bounds = speed_bounds(keys);

    // identify the minima in the speed bound partial order; only these turn into keys in the final speed curve
    let minima = sqrt_pair_minima(speed_bounds);

    // rectify from speed-distance space to distance-time space
    let time_keys = time_rectify(minima);

    splines::Spline::from_vec(time_keys)
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
        assert!(!(F1 < F2));
        assert!(!(F2 < F1));
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
