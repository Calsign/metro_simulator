use crate::address::Address;
use crate::quadrant::{QuadMap, Quadrant, QUADRANTS};
use crate::rect::Rect;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Expected branch, but got leaf")]
    ExpectedBranch(),
    #[error("Expected leaf, but got branch")]
    ExpectedLeaf(),
    #[error("Max depth exceeded: {0}")]
    MaxDepthExceeded(u32),
    #[error("Coordinates out of bounds: {0}, {1}")]
    CoordsOutOfBoundsU64(u64, u64),
    #[error("Coordinates out of bounds: {0}, {1}")]
    CoordsOutOfBoundsF64(f64, f64),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VisitData {
    pub address: Address,
    pub depth: u32,
    pub x: u64,
    pub y: u64,
    pub width: u64,
}

impl VisitData {
    pub fn in_bounds(&self, bounds: &Rect) -> bool {
        return self.x < bounds.max_x
            && self.x + self.width > bounds.min_x
            && self.y < bounds.max_y
            && self.y + self.width > bounds.min_y;
    }

    pub fn get_bounds(&self) -> Rect {
        Rect::xywh(self.x, self.y, self.width, self.width)
    }

    pub fn child(&self, quadrant: Quadrant) -> Self {
        use Quadrant::*;
        let x = match quadrant {
            NW | SW => self.x,
            NE | SE => self.x + self.width / 2,
        };
        let y = match quadrant {
            NW | NE => self.y,
            SW | SE => self.y + self.width / 2,
        };
        return Self {
            address: self.address.child(quadrant),
            depth: self.depth + 1,
            x,
            y,
            width: self.width / 2,
        };
    }

    pub fn quadrant_for_coords(&self, x: u64, y: u64) -> Result<Quadrant, Error> {
        if x < self.x || x > self.x + self.width || y < self.y || y > self.y + self.width {
            Err(Error::CoordsOutOfBoundsU64(x, y))
        } else {
            let right = x > self.x + self.width / 2;
            let bottom = y > self.y + self.width / 2;
            Ok(Quadrant::from_sides(right, bottom))
        }
    }
}

pub trait Visitor<B, L, E> {
    fn visit_branch_pre(&mut self, branch: &B, data: &VisitData) -> Result<bool, E>;
    fn visit_leaf(&mut self, leaf: &L, data: &VisitData) -> Result<(), E>;
    fn visit_branch_post(&mut self, branch: &B, data: &VisitData) -> Result<(), E>;
}

pub trait MutVisitor<B, L, E> {
    fn visit_branch_pre(&mut self, branch: &mut B, data: &VisitData) -> Result<bool, E>;
    fn visit_leaf(&mut self, leaf: &mut L, data: &VisitData) -> Result<(), E>;
    fn visit_branch_post(&mut self, branch: &mut B, data: &VisitData) -> Result<(), E>;
}

pub trait Fold<B, L, T, E> {
    fn fold_leaf(&mut self, leaf: &L, data: &VisitData) -> Result<T, E>;
    fn fold_branch(&mut self, branch: &B, children: &QuadMap<T>, data: &VisitData) -> Result<T, E>;
}

pub trait MutFold<B, L, T, E> {
    fn fold_leaf(&mut self, leaf: &mut L, data: &VisitData) -> Result<T, E>;
    fn fold_branch(
        &mut self,
        branch: &mut B,
        children: &QuadMap<T>,
        data: &VisitData,
    ) -> Result<T, E>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum Node<B, L> {
    Branch {
        data: B,
        children: QuadMap<Box<Node<B, L>>>,

        /** The number of branches above this node, up to and including the root. */
        depth: usize,
        /** The number of leaves below this branch. */
        child_count: usize,
        /**
         * The length of the longest path of branches below this node, including this node.
         * If the immediate children are all leaves, then the child_depth is 1.
         */
        child_depth: usize,
    },
    Leaf {
        data: L,

        /** The number of branches above this node, up to and including the root. */
        depth: usize,
    },
}

impl<B, L> Node<B, L> {
    fn get(&self, quadrant: Quadrant) -> Result<&Box<Node<B, L>>, Error> {
        return if let Node::Branch { children, .. } = self {
            Ok(&children[quadrant])
        } else {
            Err(Error::ExpectedBranch().into())
        };
    }

    fn get_mut(&mut self, quadrant: Quadrant) -> Result<&mut Box<Node<B, L>>, Error> {
        return if let Node::Branch { children, .. } = self {
            Ok(&mut children[quadrant])
        } else {
            Err(Error::ExpectedBranch().into())
        };
    }

    fn visit<V, E>(&self, visitor: &mut V, visit_data: VisitData) -> Result<(), E>
    where
        V: Visitor<B, L, E>,
    {
        match self {
            Node::Branch { data, children, .. } => {
                if visitor.visit_branch_pre(data, &visit_data)? {
                    for quadrant in QUADRANTS {
                        children[quadrant].visit(visitor, visit_data.child(quadrant))?;
                    }
                }
                visitor.visit_branch_post(data, &visit_data)?;
            }
            Node::Leaf { data, .. } => visitor.visit_leaf(data, &visit_data)?,
        }
        Ok(())
    }

    fn visit_mut<V, E>(&mut self, visitor: &mut V, visit_data: VisitData) -> Result<(), E>
    where
        V: MutVisitor<B, L, E>,
    {
        match self {
            Node::Branch { data, children, .. } => {
                if visitor.visit_branch_pre(data, &visit_data)? {
                    for quadrant in QUADRANTS {
                        children[quadrant].visit_mut(visitor, visit_data.child(quadrant))?;
                    }
                }
                visitor.visit_branch_post(data, &visit_data)?;
            }
            Node::Leaf { data, .. } => visitor.visit_leaf(data, &visit_data)?,
        }
        Ok(())
    }

    fn fold<F, T, E>(&self, fold: &mut F, visit_data: VisitData) -> Result<T, E>
    where
        F: Fold<B, L, T, E>,
    {
        match self {
            Node::Leaf { data, .. } => fold.fold_leaf(data, &visit_data),
            Node::Branch { data, children, .. } => {
                // TODO: this is gross
                let nw = children[Quadrant::NW].fold(fold, visit_data.child(Quadrant::NW))?;
                let ne = children[Quadrant::NE].fold(fold, visit_data.child(Quadrant::NE))?;
                let sw = children[Quadrant::SW].fold(fold, visit_data.child(Quadrant::SW))?;
                let se = children[Quadrant::SE].fold(fold, visit_data.child(Quadrant::SE))?;
                fold.fold_branch(data, &QuadMap::new(nw, ne, sw, se), &visit_data)
            }
        }
    }

    fn fold_mut<F, T, E>(&mut self, fold: &mut F, visit_data: VisitData) -> Result<T, E>
    where
        F: MutFold<B, L, T, E>,
    {
        match self {
            Node::Leaf { data, .. } => fold.fold_leaf(data, &visit_data),
            Node::Branch { data, children, .. } => {
                // TODO: this is gross
                let nw = children[Quadrant::NW].fold_mut(fold, visit_data.child(Quadrant::NW))?;
                let ne = children[Quadrant::NE].fold_mut(fold, visit_data.child(Quadrant::NE))?;
                let sw = children[Quadrant::SW].fold_mut(fold, visit_data.child(Quadrant::SW))?;
                let se = children[Quadrant::SE].fold_mut(fold, visit_data.child(Quadrant::SE))?;
                fold.fold_branch(data, &QuadMap::new(nw, ne, sw, se), &visit_data)
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Quadtree<B, L> {
    /** The root node */
    root: Box<Node<B, L>>,
    /** The maximum allowable depth of nodes below the root node */
    max_depth: u32,
    /**
     * The width of the grid if all nodes are fully expanded out to max_depth.
     * Equivalent to 2^max_depth.
     */
    width: u64,
}

impl<B, L> Quadtree<B, L> {
    pub fn new(data: L, max_depth: u32) -> Quadtree<B, L> {
        let base: u64 = 2;
        // NOTE: if the exponent is too big, we panic.
        let width = base.checked_pow(max_depth).unwrap();

        return Quadtree {
            root: Box::new(Node::Leaf { data, depth: 0 }),
            // NOTE: max_depth invariant is maintained because it is unsigned.
            // This try_into should succeed on both 32-bit and 64-bit systems.
            max_depth: max_depth.try_into().unwrap(),
            width,
        };
    }

    pub fn width(&self) -> u64 {
        self.width
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    fn get(&self, address: &Address) -> Result<&Node<B, L>, Error> {
        // NOTE: this is an associated function rather than a method to avoid borrowing the arena
        let mut node = &self.root;
        for index in 0..address.depth() {
            node = node.get(address.at(index))?;
        }
        return Ok(node);
    }

    fn get_mut(&mut self, address: &Address) -> Result<&mut Node<B, L>, Error> {
        // NOTE: this is an associated function rather than a method to avoid borrowing the arena
        let mut node = &mut self.root;
        for index in 0..address.depth() {
            node = node.get_mut(address.at(index))?;
        }
        return Ok(node);
    }

    pub fn get_branch<A: Into<Address>>(&self, address: A) -> Result<&B, Error> {
        return if let Node::Branch { data, .. } = self.get(&address.into())? {
            Ok(data)
        } else {
            Err(Error::ExpectedBranch())
        };
    }

    pub fn get_branch_mut<A: Into<Address>>(&mut self, address: A) -> Result<&mut B, Error> {
        return if let Node::Branch { data, .. } = self.get_mut(&address.into())? {
            Ok(data)
        } else {
            Err(Error::ExpectedBranch())
        };
    }

    pub fn get_leaf<A: Into<Address>>(&self, address: A) -> Result<&L, Error> {
        return if let Node::Leaf { data, .. } = self.get(&address.into())? {
            Ok(data)
        } else {
            Err(Error::ExpectedLeaf())
        };
    }

    pub fn get_leaf_mut<A: Into<Address>>(&mut self, address: A) -> Result<&mut L, Error> {
        return if let Node::Leaf { data, .. } = self.get_mut(&address.into())? {
            Ok(data)
        } else {
            Err(Error::ExpectedLeaf())
        };
    }

    pub fn split<A: Into<Address>>(
        &mut self,
        address: A,
        data: B,
        child_data: QuadMap<L>,
    ) -> Result<(), Error> {
        let address = address.into();
        let new_depth = address.depth() + 1;
        if new_depth > self.max_depth as usize {
            return Err(Error::MaxDepthExceeded(self.max_depth));
        }

        let existing = self.get_mut(&address)?;
        match existing {
            Node::Branch { .. } => {
                return Err(Error::ExpectedLeaf());
            }
            Node::Leaf {
                data: existing_data,
                depth: existing_depth,
            } => {
                *existing = Node::Branch {
                    data,
                    children: child_data.map_into(&|data| {
                        Box::new(Node::Leaf {
                            data,
                            depth: new_depth,
                        })
                    }),
                    depth: *existing_depth,
                    child_count: 4,
                    child_depth: 1,
                };

                // Update the parents' information.
                // NOTE: important to only update these after we succeed above
                // because the caller could handle or ignore the error.
                let mut node = &mut self.root;
                for index in 0..address.depth() {
                    if let Node::Branch {
                        mut child_count,
                        mut child_depth,
                        ..
                    } = **node
                    {
                        child_count += 3;
                        child_depth = std::cmp::max(child_depth, new_depth - index);
                    } else {
                        panic!("should be impossible");
                    }
                    node = node.get_mut(address.at(index))?;
                }

                return Ok(());
            }
        }
    }

    pub fn get_visit_data(&self, x: u64, y: u64) -> Result<VisitData, Error> {
        if x >= self.width || y >= self.width {
            return Err(Error::CoordsOutOfBoundsU64(x, y));
        }
        let mut address = Vec::new();
        // perform binary search in two dimensions
        let mut node = &self.root;
        let mut min_x = 0;
        let mut max_x = self.width;
        let mut min_y = 0;
        let mut max_y = self.width;
        for _depth in 0..=self.max_depth {
            match &**node {
                Node::Leaf { .. } => {
                    let depth = address.len() as u32;
                    return Ok(VisitData {
                        address: address.into(),
                        depth,
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                    });
                }
                Node::Branch { children, .. } => {
                    let center_x = (max_x + min_x) / 2;
                    let center_y = (max_y + min_y) / 2;

                    let right = x >= center_x;
                    let bottom = y >= center_y;

                    if right {
                        min_x = center_x;
                    } else {
                        max_x = center_x;
                    }
                    if bottom {
                        min_y = center_y;
                    } else {
                        max_y = center_y;
                    }

                    let quadrant = Quadrant::from_sides(right, bottom);
                    address.push(quadrant);
                    node = &children[quadrant];
                }
            }
        }
        panic!("invariant violated; nodes nested deeper than max_depth");
    }

    pub fn get_address(&self, x: u64, y: u64) -> Result<Address, Error> {
        Ok(self.get_visit_data(x, y)?.address)
    }

    fn root_visit_data(&self) -> VisitData {
        VisitData {
            depth: 0,
            x: 0,
            y: 0,
            width: self.width,
            address: vec![].into(),
        }
    }

    pub fn visit<V, E>(&self, visitor: &mut V) -> Result<(), E>
    where
        V: Visitor<B, L, E>,
    {
        self.root.visit(visitor, self.root_visit_data())
    }

    pub fn visit_mut<V, E>(&mut self, visitor: &mut V) -> Result<(), E>
    where
        V: MutVisitor<B, L, E>,
    {
        self.root.visit_mut(visitor, self.root_visit_data())
    }

    pub fn visit_rect<V, E>(&self, visitor: &mut V, bounds: &Rect) -> Result<(), E>
    where
        V: Visitor<B, L, E>,
    {
        self.visit(&mut RectVisitor {
            bounds,
            inner: visitor,
            phantom: std::marker::PhantomData::default(),
        })
    }

    pub fn visit_rect_mut<V, E>(&mut self, visitor: &mut V, bounds: &Rect) -> Result<(), E>
    where
        V: MutVisitor<B, L, E>,
    {
        self.visit_mut(&mut MutRectVisitor {
            bounds,
            inner: visitor,
            phantom: std::marker::PhantomData::default(),
        })
    }

    pub fn fold<F, T, E>(&self, fold: &mut F) -> Result<T, E>
    where
        F: Fold<B, L, T, E>,
    {
        self.root.fold(fold, self.root_visit_data())
    }

    pub fn fold_mut<F, T, E>(&mut self, fold: &mut F) -> Result<T, E>
    where
        F: MutFold<B, L, T, E>,
    {
        self.root.fold_mut(fold, self.root_visit_data())
    }
}

struct RectVisitor<'a, 'b, V, B, L, E>
where
    V: Visitor<B, L, E>,
{
    bounds: &'a Rect,
    inner: &'b mut V,
    phantom: std::marker::PhantomData<(B, L, E)>,
}

impl<'a, 'b, V, B, L, E> Visitor<B, L, E> for RectVisitor<'a, 'b, V, B, L, E>
where
    V: Visitor<B, L, E>,
{
    fn visit_branch_pre(&mut self, branch: &B, data: &VisitData) -> Result<bool, E> {
        Ok(self.inner.visit_branch_pre(branch, data)? && data.in_bounds(self.bounds))
    }

    fn visit_leaf(&mut self, leaf: &L, data: &VisitData) -> Result<(), E> {
        if data.in_bounds(self.bounds) {
            self.inner.visit_leaf(leaf, data)?
        }
        Ok(())
    }

    fn visit_branch_post(&mut self, branch: &B, data: &VisitData) -> Result<(), E> {
        Ok(self.inner.visit_branch_post(branch, data)?)
    }
}

struct MutRectVisitor<'a, 'b, V, B, L, E>
where
    V: MutVisitor<B, L, E>,
{
    bounds: &'a Rect,
    inner: &'b mut V,
    phantom: std::marker::PhantomData<(B, L, E)>,
}

impl<'a, 'b, V, B, L, E> MutVisitor<B, L, E> for MutRectVisitor<'a, 'b, V, B, L, E>
where
    V: MutVisitor<B, L, E>,
{
    fn visit_branch_pre(&mut self, branch: &mut B, data: &VisitData) -> Result<bool, E> {
        Ok(self.inner.visit_branch_pre(branch, data)? && data.in_bounds(self.bounds))
    }

    fn visit_leaf(&mut self, leaf: &mut L, data: &VisitData) -> Result<(), E> {
        if data.in_bounds(self.bounds) {
            self.inner.visit_leaf(leaf, data)?
        }
        Ok(())
    }

    fn visit_branch_post(&mut self, branch: &mut B, data: &VisitData) -> Result<(), E> {
        Ok(self.inner.visit_branch_post(branch, data)?)
    }
}

#[cfg(test)]
mod tests {
    use quadtree::*;
    use test_util::assert_equal_vec_unordered;

    #[test]
    fn create() {
        let qtree: Quadtree<(), i32> = Quadtree::new(42, 0);
        assert_eq!(qtree.get_leaf(vec!()), Ok(&42));
    }

    #[test]
    fn modify() {
        let mut qtree: Quadtree<(), i32> = Quadtree::new(42, 0);
        *qtree.get_leaf_mut(vec![]).unwrap() = 43;
        assert_eq!(qtree.get_leaf(vec!()), Ok(&43));
    }

    #[test]
    fn split() {
        let root = "root";
        let mut qtree = Quadtree::new(0, 1);
        qtree.split(vec![], root, QuadMap::new(1, 2, 3, 4)).unwrap();

        assert_eq!(qtree.get_branch(vec!()), Ok(&root));
        assert_eq!(qtree.get_leaf(vec!(Quadrant::NW)), Ok(&1));
        assert_eq!(qtree.get_leaf(vec!(Quadrant::NE)), Ok(&2));
        assert_eq!(qtree.get_leaf(vec!(Quadrant::SW)), Ok(&3));
        assert_eq!(qtree.get_leaf(vec!(Quadrant::SE)), Ok(&4));
    }

    #[test]
    fn get_mut() {
        let mut root = "root";
        let mut qtree = Quadtree::new(0, 1);
        qtree.split(vec![], root, QuadMap::new(1, 2, 3, 4)).unwrap();

        assert_eq!(qtree.get_branch_mut(vec!()), Ok(&mut root));
        assert_eq!(qtree.get_leaf_mut(vec!(Quadrant::NW)), Ok(&mut 1));
        assert_eq!(qtree.get_leaf_mut(vec!(Quadrant::NE)), Ok(&mut 2));
        assert_eq!(qtree.get_leaf_mut(vec!(Quadrant::SW)), Ok(&mut 3));
        assert_eq!(qtree.get_leaf_mut(vec!(Quadrant::SE)), Ok(&mut 4));
    }

    #[test]
    fn max_depth() {
        let mut qtree = Quadtree::new(0, 0);
        assert_eq!(
            qtree.split(vec![], 0, QuadMap::new(0, 0, 0, 0)),
            Err(Error::MaxDepthExceeded(0))
        );
    }

    #[test]
    fn get_address() {
        let mut qtree = Quadtree::new(0, 2);
        assert_eq!(qtree.get_address(0, 0), Ok(vec!().into()));
        assert_eq!(qtree.get_address(2, 2), Ok(vec!().into()));

        qtree.split(vec![], 0, QuadMap::new(1, 2, 3, 4)).unwrap();
        assert_eq!(qtree.get_address(0, 0), Ok(vec!(Quadrant::NW).into()));
        assert_eq!(qtree.get_address(2, 0), Ok(vec!(Quadrant::NE).into()));
        assert_eq!(qtree.get_address(0, 2), Ok(vec!(Quadrant::SW).into()));
        assert_eq!(qtree.get_address(2, 2), Ok(vec!(Quadrant::SE).into()));

        assert_eq!(
            qtree.get_address(4, 4),
            Err(Error::CoordsOutOfBoundsU64(4, 4))
        );

        qtree
            .split(vec![Quadrant::SE], 5, QuadMap::new(6, 7, 8, 9))
            .unwrap();
        assert_eq!(
            qtree.get_address(3, 2),
            Ok(vec!(Quadrant::SE, Quadrant::NE).into())
        );
    }

    struct SeenVisitor<B: Copy, L: Copy> {
        branches: Vec<(B, VisitData)>,
        leaves: Vec<(L, VisitData)>,
    }

    impl<B: Copy, L: Copy> SeenVisitor<B, L> {
        fn new() -> Self {
            return Self {
                branches: Vec::new(),
                leaves: Vec::new(),
            };
        }
    }

    impl<B: Copy, L: Copy> quadtree::Visitor<B, L, ()> for SeenVisitor<B, L> {
        fn visit_branch_pre(&mut self, branch: &B, data: &VisitData) -> Result<bool, ()> {
            self.branches.push((*branch, data.clone()));
            Ok(true)
        }

        fn visit_leaf(&mut self, leaf: &L, data: &VisitData) -> Result<(), ()> {
            self.leaves.push((*leaf, data.clone()));
            Ok(())
        }

        fn visit_branch_post(&mut self, branch: &B, data: &VisitData) -> Result<(), ()> {
            Ok(())
        }
    }

    fn make_visit_data(
        address: Vec<Quadrant>,
        depth: u32,
        x: u64,
        y: u64,
        width: u64,
    ) -> VisitData {
        return VisitData {
            address: address.into(),
            depth,
            x,
            y,
            width,
        };
    }

    #[test]
    fn visit1() {
        let qtree: Quadtree<i32, i32> = Quadtree::new(0, 0);
        let mut visitor = SeenVisitor::new();
        qtree.visit(&mut visitor).unwrap();

        assert_equal_vec_unordered(visitor.branches, vec![]);
        assert_equal_vec_unordered(
            visitor.leaves,
            vec![(0, make_visit_data(vec![], 0, 0, 0, 1))],
        );
    }

    #[test]
    fn visit2() {
        let mut qtree = Quadtree::new(0, 1);
        qtree.split(vec![], 0, QuadMap::new(1, 2, 3, 4)).unwrap();
        let mut visitor = SeenVisitor::new();
        qtree.visit(&mut visitor).unwrap();

        assert_equal_vec_unordered(
            visitor.branches,
            vec![(0, make_visit_data(vec![], 0, 0, 0, 2))],
        );
        assert_equal_vec_unordered(
            visitor.leaves,
            vec![
                (1, make_visit_data(vec![Quadrant::NW], 1, 0, 0, 1)),
                (2, make_visit_data(vec![Quadrant::NE], 1, 1, 0, 1)),
                (3, make_visit_data(vec![Quadrant::SW], 1, 0, 1, 1)),
                (4, make_visit_data(vec![Quadrant::SE], 1, 1, 1, 1)),
            ],
        );
    }

    #[test]
    fn visit3() {
        let mut qtree = Quadtree::new(0, 2);
        qtree.split(vec![], 0, QuadMap::new(1, 2, 3, 4)).unwrap();
        qtree
            .split(vec![Quadrant::NE], 5, QuadMap::new(6, 7, 8, 9))
            .unwrap();
        let mut visitor = SeenVisitor::new();
        qtree.visit(&mut visitor).unwrap();

        use Quadrant::*;
        assert_equal_vec_unordered(
            visitor.branches,
            vec![
                (0, make_visit_data(vec![], 0, 0, 0, 4)),
                (5, make_visit_data(vec![NE], 1, 2, 0, 2)),
            ],
        );
        assert_equal_vec_unordered(
            visitor.leaves,
            vec![
                (1, make_visit_data(vec![NW], 1, 0, 0, 2)),
                (6, make_visit_data(vec![NE, NW], 2, 2, 0, 1)),
                (7, make_visit_data(vec![NE, NE], 2, 3, 0, 1)),
                (8, make_visit_data(vec![NE, SW], 2, 2, 1, 1)),
                (9, make_visit_data(vec![NE, SE], 2, 3, 1, 1)),
                (3, make_visit_data(vec![SW], 1, 0, 2, 2)),
                (4, make_visit_data(vec![SE], 1, 2, 2, 2)),
            ],
        );
    }
}
