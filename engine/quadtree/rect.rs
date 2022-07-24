#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub min_x: u64,
    pub max_x: u64,
    pub min_y: u64,
    pub max_y: u64,
}

impl Rect {
    pub fn xywh(x: u64, y: u64, w: u64, h: u64) -> Self {
        Self {
            min_x: x,
            max_x: x + w,
            min_y: y,
            max_y: y + h,
        }
    }

    pub fn corners(ulx: u64, uly: u64, brx: u64, bry: u64) -> Self {
        Self {
            min_x: ulx,
            max_x: brx,
            min_y: uly,
            max_y: bry,
        }
    }

    pub fn contains(&self, x: u64, y: u64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.max_x > other.min_x
            && self.min_x < other.max_x
            && self.max_y > other.min_y
            && self.min_y < other.max_y
    }

    pub fn and(&self, other: &Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            max_x: self.max_x.max(other.max_x),
            min_y: self.min_y.min(other.min_y),
            max_y: self.max_y.max(other.max_y),
        }
    }
}
