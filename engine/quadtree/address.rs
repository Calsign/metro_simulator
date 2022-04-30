use crate::quadrant::Quadrant;

const MAX_ADDRESS_DEPTH: usize = 16;

#[derive(Hash, PartialEq, Eq, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct Address {
    /// the contents of the address; everything indexed to the right of `depth` is garbage
    data: [Quadrant; MAX_ADDRESS_DEPTH],
    /// the number of elements in this address
    depth: u32,
    /// the maximum depth of this address (imposed by the parent quadtree)
    max_depth: u32,
}

impl Address {
    pub fn depth(&self) -> usize {
        self.depth as usize
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub fn at(&self, index: usize) -> Quadrant {
        assert!(
            index < self.depth(),
            "index >= depth; index: {}, depth: {}",
            index,
            self.depth()
        );
        self.data[index]
    }

    pub fn has(&self, index: usize) -> bool {
        index >= 0 && index < self.depth()
    }

    pub fn from_vec(data: Vec<Quadrant>, max_depth: u32) -> Self {
        assert!(
            max_depth < MAX_ADDRESS_DEPTH as u32,
            "max_depth >= MAX_ADDRESS_DEPTH; max_depth: {}, MAX_ADDRESS_DEPTH: {}",
            max_depth,
            MAX_ADDRESS_DEPTH
        );
        assert!(
            data.len() < max_depth as usize + 1,
            "data.len() >= max_depth; data.len(): {}, max_depth: {}",
            data.len(),
            max_depth
        );
        let mut array = [Quadrant::NW; MAX_ADDRESS_DEPTH];
        array[0..data.len()].copy_from_slice(&data[..]);
        Self {
            data: array,
            depth: data.len() as u32,
            max_depth,
        }
    }

    pub fn try_from(address: &Vec<u8>, max_depth: u32) -> Option<Self> {
        let mut vec = Vec::new();
        for index in address.iter() {
            vec.push(Quadrant::try_from(*index)?);
        }
        Some(Self::from_vec(vec, max_depth))
    }

    pub fn to_vec(self) -> Vec<u8> {
        let mut vec = Vec::new();
        for quad in &self.data[..self.depth()] {
            vec.push(u8::from(quad));
        }
        vec
    }

    pub fn child(&self, quadrant: Quadrant) -> Self {
        assert!(
            self.depth() < self.max_depth as usize,
            "depth >= max_depth; depth: {}, max_depth: {}",
            self.depth(),
            self.max_depth,
        );
        let mut data = self.data.clone();
        data[self.depth()] = quadrant;
        return Self {
            data,
            depth: self.depth() as u32 + 1,
            max_depth: self.max_depth,
        };
    }

    /**
     * Returns the (x, y) coordinates of the center of the tile
     * represented by this address.
     */
    pub fn to_xy(&self) -> (u64, u64) {
        let mut x = 0;
        let mut y = 0;
        let mut w = 2_u64.pow(self.max_depth());
        for quadrant in &self.data[..self.depth()] {
            let (right, bottom) = match quadrant {
                Quadrant::NW => (false, false),
                Quadrant::NE => (true, false),
                Quadrant::SW => (false, true),
                Quadrant::SE => (true, true),
            };
            w /= 2;
            x += (right as u64) * w;
            y += (bottom as u64) * w;
        }
        // center of tile
        (x + w / 2, y + w / 2)
    }

    pub fn to_xy_f64(&self) -> (f64, f64) {
        let (x, y) = self.to_xy();
        (x as f64, y as f64)
    }

    pub fn from_xy(x: u64, y: u64, max_depth: u32) -> Self {
        Self::from_xy_depth(x, y, max_depth, max_depth)
    }

    pub fn from_xy_depth(x: u64, y: u64, depth: u32, max_depth: u32) -> Self {
        assert!(
            max_depth < MAX_ADDRESS_DEPTH as u32,
            "max_depth >= MAX_ADDRESS_DEPTH; max_depth: {}, MAX_ADDRESS_DEPTH: {}",
            max_depth,
            MAX_ADDRESS_DEPTH
        );
        assert!(depth <= max_depth);
        let w = 2_u64.pow(max_depth);
        assert!(x < w && y < w, "width: {}, x: {}, y: {}", w, x, y);
        let mut data = [Quadrant::NW; MAX_ADDRESS_DEPTH];
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0, w, 0, w);
        for i in 0..depth {
            let (mid_x, mid_y) = ((max_x + min_x) / 2, (max_y + min_y) / 2);
            let (right, bottom) = (x >= mid_x, y >= mid_y);
            data[i as usize] = Quadrant::from_sides(right, bottom);
            if right {
                min_x = mid_x;
            } else {
                max_x = mid_x;
            }
            if bottom {
                min_y = mid_y;
            } else {
                max_y = mid_y;
            }
        }
        Self {
            data,
            depth,
            max_depth,
        }
    }
}

impl From<Address> for Vec<Quadrant> {
    fn from(address: Address) -> Self {
        address.data[..address.depth()].to_vec()
    }
}

impl From<Address> for Vec<u8> {
    fn from(address: Address) -> Self {
        address.to_vec()
    }
}

impl From<(Vec<Quadrant>, u32)> for Address {
    fn from((data, max_depth): (Vec<Quadrant>, u32)) -> Self {
        Self::from_vec(data, max_depth)
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // don't show the garbage data
        f.debug_struct("Address")
            .field("data", &self.data[0..(self.depth as usize)].iter())
            .field("max_depth", &self.max_depth)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use quadtree::*;
    use Quadrant::*;

    #[test]
    fn empty() {
        let address = Address::from_vec(vec![], 3);
        assert_eq!(address.depth(), 0);
        assert_eq!(address.max_depth(), 3);
        assert_eq!(address.has(0), false);
        let vec: Vec<Quadrant> = address.into();
        assert_eq!(vec, vec![]);
    }

    #[test]
    fn simple() {
        let address = Address::from_vec(vec![NW, NE], 3);
        assert_eq!(address.depth(), 2);
        assert_eq!(address.max_depth(), 3);
        assert_eq!(address.has(0), true);
        assert_eq!(address.has(1), true);
        assert_eq!(address.has(2), false);
        let vec: Vec<Quadrant> = address.into();
        assert_eq!(vec, vec![NW, NE]);
    }

    #[test]
    fn child() {
        let zero = Address::from_vec(vec![], 3);

        let one = zero.child(NE);
        let one_vec: Vec<Quadrant> = one.clone().into();
        assert_eq!(one_vec, vec![NE]);

        let two = one.child(SW);
        let two_vec: Vec<Quadrant> = two.into();
        assert_eq!(two_vec, vec![NE, SW]);
    }

    #[test]
    fn to_xy() {
        assert_eq!(Address::from_vec(vec![NW, NW, NW], 3).to_xy(), (0, 0));
        assert_eq!(Address::from_vec(vec![NW, NE, NW], 3).to_xy(), (2, 0));
        assert_eq!(Address::from_vec(vec![SE, SE, SE], 3).to_xy(), (7, 7));
        assert_eq!(Address::from_vec(vec![SE, SE], 3).to_xy(), (7, 7));
        assert_eq!(Address::from_vec(vec![SE], 3).to_xy(), (6, 6));
        assert_eq!(
            Address::from_vec(vec![NE, SE, NW, SW, NW, SW, NW, SE, SW, SW, NW, NW], 12).to_xy(),
            (3088, 1372)
        );
    }

    #[test]
    fn from_xy() {
        assert_eq!(
            Address::from_xy(0, 0, 3),
            Address::from_vec(vec![NW, NW, NW], 3),
        );
        assert_eq!(
            Address::from_xy(4, 4, 3),
            Address::from_vec(vec![SE, NW, NW], 3),
        );
        assert_eq!(
            Address::from_xy(7, 7, 3),
            Address::from_vec(vec![SE, SE, SE], 3),
        );
        assert_eq!(
            Address::from_xy(2, 6, 3),
            Address::from_vec(vec![SW, SE, NW], 3),
        );
        assert_eq!(
            Address::from_xy(6, 2, 3),
            Address::from_vec(vec![NE, SE, NW], 3),
        );
        assert_eq!(
            Address::from_xy(3088, 1372, 12),
            (vec![NE, SE, NW, SW, NW, SW, NW, SE, SW, SW, NW, NW], 12).into(),
        );
    }

    #[test]
    fn from_xy_depth() {
        assert_eq!(
            Address::from_xy_depth(7, 7, 1, 3),
            Address::from_vec(vec![SE], 3)
        );
        assert_eq!(
            Address::from_xy_depth(4, 4, 2, 3),
            Address::from_vec(vec![SE, NW], 3)
        );
    }
}
