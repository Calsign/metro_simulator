#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Quadrant {
    NW = 0,
    NE = 1,
    SW = 2,
    SE = 3,
}

impl Quadrant {
    fn index(self) -> u8 {
        self as u8
    }

    pub fn try_from(index: u8) -> Option<Self> {
        use Quadrant::*;
        return match index {
            0 => Some(NW),
            1 => Some(NE),
            2 => Some(SW),
            3 => Some(SE),
            _ => None,
        };
    }

    pub fn from_sides(right: bool, bottom: bool) -> Self {
        use Quadrant::*;
        return match (right, bottom) {
            (false, false) => NW,
            (true, false) => NE,
            (false, true) => SW,
            (true, true) => SE,
        };
    }
}

impl From<&Quadrant> for u8 {
    fn from(quadrant: &Quadrant) -> Self {
        quadrant.index()
    }
}

pub static QUADRANTS: [Quadrant; 4] = [Quadrant::NW, Quadrant::NE, Quadrant::SW, Quadrant::SE];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuadMap<T> {
    data: [T; 4],
}

impl<T> QuadMap<T> {
    pub fn new(nw: T, ne: T, sw: T, se: T) -> Self {
        return QuadMap {
            data: [nw, ne, sw, se],
        };
    }

    pub fn map_into<U>(self, f: &dyn Fn(T) -> U) -> QuadMap<U> {
        return QuadMap {
            data: self.data.map(f),
        };
    }
}

impl<T> std::ops::Index<Quadrant> for QuadMap<T> {
    type Output = T;
    fn index(&self, quadrant: Quadrant) -> &T {
        return &self.data[quadrant.index() as usize];
    }
}

impl<T> std::ops::IndexMut<Quadrant> for QuadMap<T> {
    fn index_mut(&mut self, quadrant: Quadrant) -> &mut T {
        return &mut self.data[quadrant.index() as usize];
    }
}

#[cfg(test)]
mod tests {
    use crate::quadrant::*;

    #[test]
    fn quadrant_map() {
        let map = QuadMap::new(0, 1, 2, 3);
        assert_eq!(map[Quadrant::NW], 0);
        assert_eq!(map[Quadrant::NE], 1);
        assert_eq!(map[Quadrant::SW], 2);
        assert_eq!(map[Quadrant::SE], 3);
    }

    #[test]
    fn quadrant_map_mut() {
        let mut map = QuadMap::new(0, 1, 2, 3);
        map[Quadrant::NW] = 5;
        assert_eq!(map[Quadrant::NW], 5);
    }
}
