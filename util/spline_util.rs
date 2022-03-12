use cgmath as cg;

pub trait SplineVisitor<T, E> {
    fn visit(&mut self, line: &T, vertex: cg::Vector2<f64>, t: f64) -> Result<(), E>;
}

pub fn visit_spline<T, V, E>(
    owner: &T,
    spline: &splines::Spline<f64, cg::Vector2<f64>>,
    length: f64,
    visitor: &mut V,
    step: f64,
    rect: &quadtree::Rect,
) -> Result<(), E>
where
    V: SplineVisitor<T, E>,
{
    if spline.len() == 0 {
        return Ok(());
    }

    let (min_x, max_x, min_y, max_y) = (
        rect.min_x as f64,
        rect.max_x as f64,
        rect.min_y as f64,
        rect.max_y as f64,
    );

    let cx = (max_x + min_x) / 2.0;
    let cy = (max_y + min_y) / 2.0;
    let rx = (max_x - min_x) / 2.0;
    let ry = (max_y - min_y) / 2.0;

    let total = (length / step).ceil() as u64;
    let mut i = 0;
    while i <= total {
        // probe for points in the rectangle
        let (point, t) = loop {
            let t = (i as f64) * step;
            let point = spline.clamped_sample(t).unwrap();
            // compute Manhatten distance between point and rectangle
            let dist = f64::min(f64::abs(point.x - cx) - rx, f64::abs(point.y - cy) - ry);
            if dist <= step || i > total {
                i += 1;
                break (point, t);
            } else {
                i += f64::max(f64::floor(dist / step), 1.0) as u64;
            }
        };

        visitor.visit(owner, point, t)?;
    }
    Ok(())
}
