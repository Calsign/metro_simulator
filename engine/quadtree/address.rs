use crate::quadrant::Quadrant;

#[derive(Debug, Hash, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Address {
    data: Vec<Quadrant>,
    max_depth: u32,
}

impl Address {
    pub fn depth(&self) -> usize {
        self.data.len()
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub fn at(&self, index: usize) -> Quadrant {
        self.data[index]
    }

    pub fn has(&self, index: usize) -> bool {
        index > 0 && index < self.depth()
    }

    pub fn from_vec(data: Vec<Quadrant>, max_depth: u32) -> Self {
        assert!(
            data.len() < max_depth as usize + 1,
            "{}, {}",
            data.len(),
            max_depth
        );
        Self { data, max_depth }
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
        for quad in self.data.iter() {
            vec.push(u8::from(quad));
        }
        vec
    }

    pub fn child(&self, quadrant: Quadrant) -> Self {
        let mut address = self.data.clone();
        address.push(quadrant);
        return Self::from_vec(address, self.max_depth);
    }

    /**
     * Returns the (x, y) coordinates of the center of the tile
     * represented by this address.
     */
    pub fn to_xy(&self) -> (u64, u64) {
        let mut x = 0;
        let mut y = 0;
        let mut w = 2_u64.pow(self.max_depth());
        for quadrant in self.data.iter() {
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
}

impl From<Address> for Vec<Quadrant> {
    fn from(address: Address) -> Self {
        address.data
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

#[cfg(test)]
mod tests {
    use quadtree::*;
    use Quadrant::*;

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
}
