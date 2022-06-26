#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Quadrant {
    NW = 0,
    NE = 1,
    SW = 2,
    SE = 3,
}

impl Quadrant {
    pub fn index(self) -> u8 {
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
        Self {
            data: [nw, ne, sw, se],
        }
    }

    pub fn each<F>(f: F) -> Self
    where
        F: Fn() -> T,
    {
        Self {
            data: [f(), f(), f(), f()],
        }
    }

    pub fn map_into<F, U>(self, f: &F) -> QuadMap<U>
    where
        F: Fn(T) -> U,
    {
        QuadMap {
            data: self.data.map(f),
        }
    }

    pub fn values(&self) -> &[T; 4] {
        &self.data
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

impl<T> From<Vec<T>> for QuadMap<T> {
    fn from(vec: Vec<T>) -> Self {
        Self {
            data: vec
                .try_into()
                .unwrap_or_else(|v: Vec<T>| panic!("vec must have size 4")),
        }
    }
}

impl<T> From<[T; 4]> for QuadMap<T> {
    fn from(arr: [T; 4]) -> Self {
        Self { data: arr }
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
