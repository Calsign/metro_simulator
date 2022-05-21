use cgmath as cg;

pub trait SplineVisitor<T, P, E> {
    fn visit(&mut self, line: &T, vertex: P, t: f64, prev: Option<P>) -> Result<(), E>;
}

pub fn visit_spline<F, T, V, E, P, G>(
    owner: &T,
    spline: &splines::Spline<F, P>,
    length: F,
    visitor: &mut V,
    step: f64,
    rect: &quadtree::Rect,
    get_pos: G,
) -> Result<(), E>
where
    F: num::Float + num::ToPrimitive + splines::interpolate::Interpolator + std::fmt::Debug,
    V: SplineVisitor<T, P, E>,
    P: splines::Interpolate<F>,
    G: Fn(P) -> cg::Vector2<F>,
{
    if spline.len() == 0 {
        return Ok(());
    }

    let two = F::from(2.0).unwrap();

    // how close together we want the samples to be
    let step = F::from(step).unwrap();
    // how far samples can be off-screen before we stop showing them
    let buffer = step * two;

    let (min_x, max_x, min_y, max_y) = (
        F::from(rect.min_x).unwrap(),
        F::from(rect.max_x).unwrap(),
        F::from(rect.min_y).unwrap(),
        F::from(rect.max_y).unwrap(),
    );

    let cx = (max_x + min_x) / two;
    let cy = (max_y + min_y) / two;
    let rx = (max_x - min_x) / two;
    let ry = (max_y - min_y) / two;

    let total = (length / step).ceil().to_u64().unwrap();
    let mut i = 0;
    let mut prev = None;

    // probe for points in the rectangle
    loop {
        let t = F::from(i).unwrap() * step;
        let data = spline.clamped_sample(t).unwrap();
        let point = get_pos(data);

        // compute Manhatten distance between point and rectangle
        let dist = F::max(F::abs(point.x - cx) - rx, F::abs(point.y - cy) - ry);

        if dist <= buffer {
            // we're on the screen
            i += 1;
            visitor.visit(owner, data, t.to_f64().unwrap(), prev)?;
            prev = Some(data);
        } else {
            // we're off the screen; skip forward
            i += F::max(F::floor(dist / step), F::one()).to_u64().unwrap();
            prev = None;
        }

        if i > total {
            break;
        }
    }
    Ok(())
}

pub fn compute_bounds<T, F>(nodes: &[T], f: F) -> quadtree::Rect
where
    F: Fn(&T) -> (f64, f64),
{
    let mut bounds = quadtree::Rect {
        min_x: u64::MAX,
        max_x: u64::MIN,
        min_y: u64::MAX,
        max_y: u64::MIN,
    };

    for node in nodes {
        let (xf, yf) = f(node);
        let (x, y) = (xf as u64, yf as u64);
        bounds.min_x = u64::min(bounds.min_x, x);
        bounds.max_x = u64::max(bounds.max_x, x);
        bounds.min_y = u64::min(bounds.min_y, y);
        bounds.max_y = u64::max(bounds.max_y, y);
    }

    bounds
}
