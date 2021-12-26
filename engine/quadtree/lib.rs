mod address;
mod quadrant;
mod rect;

pub use address::Address;
pub use quadrant::{QuadMap, Quadrant, QUADRANTS};
pub use rect::Rect;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Expected branch, but got leaf")]
    ExpectedBranch(),
    #[error("Expected leaf, but got branch")]
    ExpectedLeaf(),
    #[error("Max depth exceeded: {0}")]
    MaxDepthExceeded(usize),
    #[error("Coordinates out of bounds: {0}, {1}")]
    CoordsOutOfBounds(u64, u64),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct VisitData {
    pub depth: usize,
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
}

pub trait Visitor<B, L> {
    fn visit_branch(&mut self, branch: &B, data: &VisitData) -> bool;
    fn visit_leaf(&mut self, leaf: &L, data: &VisitData);
}

pub trait MutVisitor<B, L> {
    fn visit_branch(&mut self, branch: &mut B, data: &VisitData) -> bool;
    fn visit_leaf(&mut self, leaf: &mut L, data: &VisitData);
}

impl VisitData {
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
            depth: self.depth + 1,
            x,
            y,
            width: self.width / 2,
        };
    }
}

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

    fn visit(&self, visitor: &mut dyn Visitor<B, L>, visit_data: VisitData) {
        match self {
            Node::Branch { data, children, .. } => {
                if visitor.visit_branch(data, &visit_data) {
                    for quadrant in QUADRANTS {
                        children[quadrant].visit(visitor, visit_data.child(quadrant));
                    }
                }
            }
            Node::Leaf { data, .. } => visitor.visit_leaf(data, &visit_data),
        }
    }

    fn visit_mut(&mut self, visitor: &mut dyn MutVisitor<B, L>, visit_data: VisitData) {
        match self {
            Node::Branch { data, children, .. } => {
                if visitor.visit_branch(data, &visit_data) {
                    for quadrant in QUADRANTS {
                        children[quadrant].visit_mut(visitor, visit_data.child(quadrant));
                    }
                }
            }
            Node::Leaf { data, .. } => visitor.visit_leaf(data, &visit_data),
        }
    }
}

pub struct Quadtree<B, L> {
    /** The root node */
    root: Box<Node<B, L>>,
    /** The maximum allowable depth of nodes below the root node */
    max_depth: usize,
    /**
     * The width of the grid if all nodes are fully expanded out to max_depth.
     * Equivalent to 2^max_depth.
     */
    width: u64,
}

impl<B, L> Quadtree<B, L> {
    pub fn new(data: L, max_depth: u32) -> Quadtree<B, L> {
        use std::convert::TryInto;

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
        if new_depth > self.max_depth {
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

    pub fn get_coords(&self, x: u64, y: u64) -> Result<&L, Error> {
        if x >= self.width || y >= self.width {
            return Err(Error::CoordsOutOfBounds(x, y));
        }
        // perform binary search in two dimensions
        let mut node = &self.root;
        let mut min_x = 0;
        let mut max_x = self.width;
        let mut min_y = 0;
        let mut max_y = self.width;
        for _depth in 0..=self.max_depth {
            match &**node {
                Node::Leaf { data, .. } => return Ok(data),
                Node::Branch { children, .. } => {
                    let right = x >= (max_x - min_x) / 2;
                    let bottom = y >= (max_y - min_y) / 2;

                    if right {
                        min_x = (max_x - min_x) / 2;
                    } else {
                        max_x = (max_x - min_x) / 2;
                    }
                    if bottom {
                        min_y = (max_y - min_y) / 2;
                    } else {
                        max_y = (max_y - min_y) / 2;
                    }

                    node = &children[Quadrant::from_sides(right, bottom)]
                }
            }
        }
        panic!("invariant violated; nodes nested deeper than max_depth");
    }

    pub fn visit(&self, visitor: &mut dyn Visitor<B, L>) {
        self.root.visit(
            visitor,
            VisitData {
                depth: 0,
                x: 0,
                y: 0,
                width: self.width,
            },
        );
    }

    pub fn visit_mut(&mut self, visitor: &mut dyn MutVisitor<B, L>) {
        self.root.visit_mut(
            visitor,
            VisitData {
                depth: 0,
                x: 0,
                y: 0,
                width: self.width,
            },
        );
    }

    pub fn visit_rect(&self, visitor: &mut dyn Visitor<B, L>, bounds: &Rect) {
        self.visit(&mut RectVisitor {
            bounds,
            inner: visitor,
        });
    }

    pub fn visit_rect_mut(&mut self, visitor: &mut dyn MutVisitor<B, L>, bounds: &Rect) {
        self.visit_mut(&mut MutRectVisitor {
            bounds,
            inner: visitor,
        });
    }
}

struct RectVisitor<'a, 'b, B, L> {
    bounds: &'a Rect,
    inner: &'b mut dyn Visitor<B, L>,
}

impl<'a, 'b, B, L> Visitor<B, L> for RectVisitor<'a, 'b, B, L> {
    fn visit_branch(&mut self, branch: &B, data: &VisitData) -> bool {
        return self.inner.visit_branch(branch, data) && data.in_bounds(self.bounds);
    }

    fn visit_leaf(&mut self, leaf: &L, data: &VisitData) {
        self.inner.visit_leaf(leaf, data);
    }
}

struct MutRectVisitor<'a, 'b, B, L> {
    bounds: &'a Rect,
    inner: &'b mut dyn MutVisitor<B, L>,
}

impl<'a, 'b, B, L> MutVisitor<B, L> for MutRectVisitor<'a, 'b, B, L> {
    fn visit_branch(&mut self, branch: &mut B, data: &VisitData) -> bool {
        return self.inner.visit_branch(branch, data) && data.in_bounds(self.bounds);
    }

    fn visit_leaf(&mut self, leaf: &mut L, data: &VisitData) {
        self.inner.visit_leaf(leaf, data);
    }
}

#[cfg(test)]
mod tests {
    use quadtree::*;

    fn assert_equal_vec_unordered<T: Eq + std::fmt::Debug>(vec1: Vec<T>, vec2: Vec<T>) {
        // Without assuming anything about T besides Eq and Debug (like Hash or Ord),
        // the best we can do is O(n^2). This is OK for tests. Please don't use this
        // for non-test code.
        assert_eq!(
            vec1.len(),
            vec2.len(),
            "Vectors have different lengths: {:?}, {:?}",
            vec1,
            vec2
        );
        'outer: for item1 in vec1.iter() {
            for item2 in vec2.iter() {
                if item1 == item2 {
                    continue 'outer;
                }
            }
            assert!(
                false,
                "Vectors are not order-independent equal:\n  {:?}\n  {:?}",
                vec1, vec2
            );
        }
    }

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
    fn get_coords() {
        let mut qtree = Quadtree::new(0, 1);
        assert_eq!(qtree.get_coords(0, 0), Ok(&0));
        assert_eq!(qtree.get_coords(1, 1), Ok(&0));

        qtree.split(vec![], 0, QuadMap::new(1, 2, 3, 4)).unwrap();
        assert_eq!(qtree.get_coords(0, 0), Ok(&1));
        assert_eq!(qtree.get_coords(1, 0), Ok(&2));
        assert_eq!(qtree.get_coords(0, 1), Ok(&3));
        assert_eq!(qtree.get_coords(1, 1), Ok(&4));

        assert_eq!(qtree.get_coords(2, 2), Err(Error::CoordsOutOfBounds(2, 2)));
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

    impl<B: Copy, L: Copy> quadtree::Visitor<B, L> for SeenVisitor<B, L> {
        fn visit_branch(&mut self, branch: &B, data: &VisitData) -> bool {
            self.branches.push((*branch, *data));
            return true;
        }

        fn visit_leaf(&mut self, leaf: &L, data: &VisitData) {
            self.leaves.push((*leaf, *data));
        }
    }

    fn make_visit_data(depth: usize, x: u64, y: u64, width: u64) -> VisitData {
        return VisitData { depth, x, y, width };
    }

    #[test]
    fn visit1() {
        let mut qtree: Quadtree<i32, i32> = Quadtree::new(0, 0);
        let mut visitor = SeenVisitor::new();
        qtree.visit(&mut visitor);

        assert_equal_vec_unordered(visitor.branches, vec![]);
        assert_equal_vec_unordered(visitor.leaves, vec![(0, make_visit_data(0, 0, 0, 1))]);
    }

    #[test]
    fn visit2() {
        let mut qtree = Quadtree::new(0, 1);
        qtree.split(vec![], 0, QuadMap::new(1, 2, 3, 4)).unwrap();
        let mut visitor = SeenVisitor::new();
        qtree.visit(&mut visitor);

        assert_equal_vec_unordered(visitor.branches, vec![(0, make_visit_data(0, 0, 0, 2))]);
        assert_equal_vec_unordered(
            visitor.leaves,
            vec![
                (1, make_visit_data(1, 0, 0, 1)),
                (2, make_visit_data(1, 1, 0, 1)),
                (3, make_visit_data(1, 0, 1, 1)),
                (4, make_visit_data(1, 1, 1, 1)),
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
        qtree.visit(&mut visitor);

        assert_equal_vec_unordered(
            visitor.branches,
            vec![
                (0, make_visit_data(0, 0, 0, 4)),
                (5, make_visit_data(1, 2, 0, 2)),
            ],
        );
        assert_equal_vec_unordered(
            visitor.leaves,
            vec![
                (1, make_visit_data(1, 0, 0, 2)),
                (6, make_visit_data(2, 2, 0, 1)),
                (7, make_visit_data(2, 3, 0, 1)),
                (8, make_visit_data(2, 2, 1, 1)),
                (9, make_visit_data(2, 3, 1, 1)),
                (3, make_visit_data(1, 0, 2, 2)),
                (4, make_visit_data(1, 2, 2, 2)),
            ],
        );
    }
}
