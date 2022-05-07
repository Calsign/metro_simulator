use cgmath as cg;

pub trait SplineVisitor<T, P, E> {
    fn visit(&mut self, line: &T, vertex: P, t: f64) -> Result<(), E>;
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
    F: num::Float + num::ToPrimitive + splines::interpolate::Interpolator,
    V: SplineVisitor<T, P, E>,
    P: splines::Interpolate<F>,
    G: Fn(P) -> cg::Vector2<F>,
{
    if spline.len() == 0 {
        return Ok(());
    }

    let step = F::from(step).unwrap();

    let (min_x, max_x, min_y, max_y) = (
        F::from(rect.min_x).unwrap(),
        F::from(rect.max_x).unwrap(),
        F::from(rect.min_y).unwrap(),
        F::from(rect.max_y).unwrap(),
    );

    let two = F::from(2.0).unwrap();

    let cx = (max_x + min_x) / two;
    let cy = (max_y + min_y) / two;
    let rx = (max_x - min_x) / two;
    let ry = (max_y - min_y) / two;

    let total = (length / step).ceil().to_u64().unwrap();
    let mut i = 0;
    while i <= total {
        // probe for points in the rectangle
        let (data, t) = loop {
            let t = F::from(i).unwrap() * step;
            let data = spline.clamped_sample(t).unwrap();
            let point = get_pos(data);
            // compute Manhatten distance between point and rectangle
            let dist = F::min(F::abs(point.x - cx) - rx, F::abs(point.y - cy) - ry);
            if dist <= step || i > total {
                i += 1;
                break (data, t);
            } else {
                i += F::max(F::floor(dist / step), F::one()).to_u64().unwrap();
            }
        };

        visitor.visit(owner, data, t.to_f64().unwrap())?;
    }
    Ok(())
}
