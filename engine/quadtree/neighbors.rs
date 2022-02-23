use crate::quadrant::{QuadMap, QUADRANTS};
use crate::quadtree::{Error, Quadtree, VisitData};
use crate::rect::Rect;

struct Entry<T> {
    x: f64,
    y: f64,
    data: T,
}

/**
 * A data structure which can be used to efficiently query all
 * neighbors of one entry. Backed by a quadtree.
 */
pub struct NeighborsStore<T> {
    qtree: Quadtree<(), Vec<Entry<T>>>,
    load_factor: u32,
}

pub trait NeighborsVisitor<T, E> {
    fn visit(&mut self, entry: &T, x: f64, y: f64, distance: f64) -> Result<(), E>;
}

pub trait AllNeighborsVisitor<T, E> {
    fn visit(&mut self, base: &T, entry: &T, distance: f64) -> Result<(), E>;
}

impl<T> NeighborsStore<T> {
    pub fn new(load_factor: u32, max_depth: u32) -> Self {
        Self {
            qtree: Quadtree::new(Vec::new(), max_depth),
            load_factor,
        }
    }

    fn split_if_needed(&mut self, visit_data: VisitData) -> Result<(), Error> {
        let max_depth = self.qtree.max_depth();
        let node = self.qtree.get_leaf_mut(visit_data.address.clone())?;
        if node.len() > self.load_factor as usize && visit_data.depth < max_depth {
            // over load factor; perform splitting
            let mut quads = QuadMap::each(Vec::new);
            for entry in node.drain(0..) {
                quads[visit_data.quadrant_for_coords(entry.x as u64, entry.y as u64)?].push(entry);
            }
            self.qtree.split(visit_data.address.clone(), (), quads)?;

            // if we put all of them into one quadrant, then we need to split again
            for quadrant in QUADRANTS {
                self.split_if_needed(visit_data.child(quadrant))?;
            }
        }
        Ok(())
    }

    pub fn insert(&mut self, entry: T, x: f64, y: f64) -> Result<(), Error> {
        if x < 0.0 || x > self.qtree.width() as f64 || y < 0.0 || y > self.qtree.width() as f64 {
            Err(crate::quadtree::Error::CoordsOutOfBoundsF64(x, y))
        } else {
            let visit_data = self.qtree.get_visit_data(x as u64, y as u64)?;
            self.qtree
                .get_leaf_mut(visit_data.address.clone())?
                .push(Entry { x, y, data: entry });
            self.split_if_needed(visit_data)?;
            Ok(())
        }
    }

    pub fn visit_radius<V, E>(&self, visitor: &mut V, x: f64, y: f64, radius: f64) -> Result<(), E>
    where
        V: NeighborsVisitor<T, E>,
    {
        let mut visitor = NeighborsVisitorImpl {
            x,
            y,
            radius,
            visitor,
            phantom: std::marker::PhantomData::default(),
        };
        // center rect, rounding to the outside
        let rect = Rect::corners(
            (x - radius.ceil() / 2.0).floor() as u64,
            (y - radius.ceil() / 2.0).floor() as u64,
            (x + radius.ceil() / 2.0).ceil() as u64,
            (x + radius.ceil() / 2.0).ceil() as u64,
        );
        self.qtree.visit_rect(&mut visitor, &rect)?;
        Ok(())
    }

    pub fn visit_all_radius<V, F, E>(&self, visitor: &mut V, radius: F) -> Result<(), E>
    where
        V: AllNeighborsVisitor<T, E>,
        F: Fn(&T) -> f64,
    {
        let mut visitor = NeighborsAllRadiusVisitorImpl {
            store: self,
            radius,
            visitor,
            phantom: std::marker::PhantomData::default(),
        };
        self.qtree.visit(&mut visitor)?;
        Ok(())
    }
}

struct NeighborsVisitorImpl<'a, V, T, E>
where
    V: NeighborsVisitor<T, E>,
{
    x: f64,
    y: f64,
    radius: f64,
    visitor: &'a mut V,
    phantom: std::marker::PhantomData<(T, E)>,
}

impl<'a, V, T, E> crate::quadtree::Visitor<(), Vec<Entry<T>>, E>
    for NeighborsVisitorImpl<'a, V, T, E>
where
    V: NeighborsVisitor<T, E>,
{
    fn visit_branch_pre(&mut self, branch: &(), data: &VisitData) -> Result<bool, E> {
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &Vec<Entry<T>>, data: &VisitData) -> Result<(), E> {
        for entry in leaf {
            let distance = ((entry.x - self.x).powi(2) + (entry.y - self.y).powi(2)).sqrt();
            if distance <= self.radius {
                self.visitor
                    .visit(&entry.data, entry.x, entry.y, distance)?;
            }
        }
        Ok(())
    }

    fn visit_branch_post(&mut self, branch: &(), data: &VisitData) -> Result<(), E> {
        Ok(())
    }
}

struct AllNeighborsVisitorImpl<'a, 'b, V, T, E>
where
    V: AllNeighborsVisitor<T, E>,
{
    visitor: &'a mut V,
    base: &'b T,
    phantom: std::marker::PhantomData<E>,
}

impl<'a, 'b, V, T, E> NeighborsVisitor<T, E> for AllNeighborsVisitorImpl<'a, 'b, V, T, E>
where
    V: AllNeighborsVisitor<T, E>,
{
    fn visit(&mut self, entry: &T, _x: f64, _y: f64, distance: f64) -> Result<(), E> {
        self.visitor.visit(self.base, entry, distance)
    }
}

struct NeighborsAllRadiusVisitorImpl<'a, 'b, V, T, F, E>
where
    V: AllNeighborsVisitor<T, E>,
    F: Fn(&T) -> f64,
{
    store: &'a NeighborsStore<T>,
    radius: F,
    visitor: &'b mut V,
    phantom: std::marker::PhantomData<E>,
}

impl<'a, 'b, V, T, F, E> crate::quadtree::Visitor<(), Vec<Entry<T>>, E>
    for NeighborsAllRadiusVisitorImpl<'a, 'b, V, T, F, E>
where
    V: AllNeighborsVisitor<T, E>,
    F: Fn(&T) -> f64,
{
    fn visit_branch_pre(&mut self, branch: &(), data: &VisitData) -> Result<bool, E> {
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &Vec<Entry<T>>, data: &VisitData) -> Result<(), E> {
        for entry in leaf {
            let mut visitor = AllNeighborsVisitorImpl {
                visitor: self.visitor,
                base: &entry.data,
                phantom: std::marker::PhantomData::default(),
            };
            self.store
                .visit_radius(&mut visitor, entry.x, entry.y, (self.radius)(&entry.data))?;
        }
        Ok(())
    }

    fn visit_branch_post(&mut self, branch: &(), data: &VisitData) -> Result<(), E> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use quadtree::*;
    use test_util::assert_equal_vec_unordered;

    struct TestVisitor {
        seen: Vec<u32>,
    }

    impl TestVisitor {
        pub fn new() -> Self {
            Self { seen: Vec::new() }
        }
    }

    impl NeighborsVisitor<u32, quadtree::Error> for TestVisitor {
        fn visit(
            &mut self,
            data: &u32,
            x: f64,
            y: f64,
            distance: f64,
        ) -> Result<(), quadtree::Error> {
            self.seen.push(*data);
            Ok(())
        }
    }

    fn assert_visit_eq(
        neighbors: &NeighborsStore<u32>,
        x: f64,
        y: f64,
        radius: f64,
        seen: Vec<u32>,
    ) -> Result<(), quadtree::Error> {
        let mut visitor = TestVisitor::new();
        neighbors.visit_radius(&mut visitor, x, y, radius)?;
        assert_equal_vec_unordered(visitor.seen, seen);
        Ok(())
    }

    #[test]
    fn simple() -> Result<(), quadtree::Error> {
        let mut neighbors = NeighborsStore::new(1, 2);
        assert_visit_eq(&neighbors, 0.0, 0.0, 1.0, vec![])?;

        neighbors.insert(0, 0.0, 0.0)?;
        assert_visit_eq(&neighbors, 0.0, 0.0, 1.0, vec![0])?;

        neighbors.insert(1, 2.0, 2.0)?;
        assert_visit_eq(&neighbors, 0.0, 0.0, 1.0, vec![0])?;
        assert_visit_eq(&neighbors, 2.0, 2.0, 1.0, vec![1])?;
        assert_visit_eq(&neighbors, 1.0, 1.0, 2.0, vec![0, 1])?;
        assert_visit_eq(&neighbors, 1.0, 1.0, 1.0, vec![])?;

        Ok(())
    }

    #[test]
    fn out_of_bounds() -> Result<(), quadtree::Error> {
        let mut neighbors = NeighborsStore::new(1, 2);
        assert_eq!(
            neighbors.insert(0, -1.0, -1.0),
            Err(Error::CoordsOutOfBoundsF64(-1.0, -1.0))
        );
        assert_eq!(
            neighbors.insert(1, 5.0, 5.0),
            Err(Error::CoordsOutOfBoundsF64(5.0, 5.0))
        );
        assert_visit_eq(&neighbors, 0.0, 0.0, 10.0, vec![])?;
        Ok(())
    }

    #[test]
    fn max_depth() -> Result<(), quadtree::Error> {
        let mut neighbors = NeighborsStore::new(1, 1);
        neighbors.insert(0, 0.0, 0.0)?;
        // this one will crash if max depth isn't respected
        neighbors.insert(1, 0.0, 0.0)?;
        Ok(())
    }
}
