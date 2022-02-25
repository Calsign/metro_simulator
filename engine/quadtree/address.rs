use crate::quadrant::Quadrant;

#[derive(Debug, Hash, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Address {
    data: Vec<Quadrant>,
}

impl Address {
    pub fn depth(&self) -> usize {
        self.data.len()
    }

    pub fn at(&self, index: usize) -> Quadrant {
        self.data[index]
    }

    pub fn has(&self, index: usize) -> bool {
        index > 0 && index < self.depth()
    }

    pub fn try_from(address: &Vec<u8>) -> Option<Self> {
        let mut vec = Vec::new();
        for index in address.iter() {
            vec.push(Quadrant::try_from(*index)?);
        }
        Some(Address::from(vec))
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
        return address.into();
    }

    /**
     * Returns the (x, y) coordinates of the center of the tile
     * represented by this address in a quadtree with `max_depth`.
     */
    pub fn to_xy(&self, max_depth: u32) -> (u64, u64) {
        let mut x = 0;
        let mut y = 0;
        let mut w = 2_u64.pow(max_depth);
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

impl From<Vec<Quadrant>> for Address {
    fn from(data: Vec<Quadrant>) -> Self {
        Self { data }
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

#[cfg(test)]
mod tests {
    use quadtree::*;
    use Quadrant::*;

    #[test]
    fn to_xy() {
        assert_eq!(Address::from(vec![NW, NW, NW]).to_xy(3), (0, 0));
        assert_eq!(Address::from(vec![NW, NE, NW]).to_xy(3), (2, 0));
        assert_eq!(Address::from(vec![SE, SE, SE]).to_xy(3), (7, 7));
        assert_eq!(Address::from(vec![SE, SE]).to_xy(3), (7, 7));
        assert_eq!(Address::from(vec![SE]).to_xy(3), (6, 6));
        assert_eq!(
            Address::from(vec![NE, SE, NW, SW, NW, SW, NW, SE, SW, SW, NW, NW]).to_xy(12),
            (3088, 1372)
        );
    }
}
