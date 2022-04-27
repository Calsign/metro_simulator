use crate::address::Address;
use crate::quadrant::Quadrant;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    North,
    South,
    West,
    East,
}

pub static DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

impl Direction {
    pub fn opposite(&self) -> Self {
        match self {
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    pub fn get_quadrants(&self) -> [Quadrant; 2] {
        match self {
            Direction::North => [Quadrant::NW, Quadrant::NE],
            Direction::South => [Quadrant::SW, Quadrant::SE],
            Direction::West => [Quadrant::NW, Quadrant::SW],
            Direction::East => [Quadrant::NE, Quadrant::SE],
        }
    }
}

impl Address {
    pub fn in_direction(&self, direction: Direction) -> Option<Address> {
        // NOTE: center of tile, but that shouldn't matter
        let (x, y) = self.to_xy();
        let (ix, iy) = (x as i64, y as i64);
        let total_width = 2_i64.pow(self.max_depth());
        let width = 2_i64.pow(self.max_depth() - self.depth() as u32);
        let (nx, ny) = match &direction {
            Direction::North => (ix, iy - width),
            Direction::South => (ix, iy + width),
            Direction::West => (ix - width, iy),
            Direction::East => (ix + width, iy),
        };
        if nx >= 0 && nx < total_width && ny >= 0 && ny <= total_width {
            Some(Self::from_xy_depth(
                nx as u64,
                ny as u64,
                self.depth() as u32,
                self.max_depth(),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use quadtree::*;
    use Direction::*;
    use Quadrant::*;

    #[test]
    fn in_direction() {
        assert_eq!(
            Address::from_vec(vec![NW], 3).in_direction(East),
            Some(Address::from_vec(vec![NE], 3))
        );
        assert_eq!(
            Address::from_vec(vec![NE], 3).in_direction(West),
            Some(Address::from_vec(vec![NW], 3))
        );
        assert_eq!(
            Address::from_vec(vec![NW], 3).in_direction(South),
            Some(Address::from_vec(vec![SW], 3))
        );
        assert_eq!(
            Address::from_vec(vec![SW], 3).in_direction(North),
            Some(Address::from_vec(vec![NW], 3))
        );
        assert_eq!(Address::from_vec(vec![NW], 3).in_direction(West), None);
        assert_eq!(Address::from_vec(vec![NW], 3).in_direction(North), None);
        assert_eq!(Address::from_vec(vec![SE], 3).in_direction(East), None);
        assert_eq!(Address::from_vec(vec![SE], 3).in_direction(South), None);
        assert_eq!(
            Address::from_vec(vec![NW, SE], 3).in_direction(East),
            Some(Address::from_vec(vec![NE, SW], 3))
        );
        assert_eq!(
            Address::from_vec(vec![NW, NW, NW], 3).in_direction(South),
            Some(Address::from_vec(vec![NW, NW, SW], 3)),
        );
    }
}
