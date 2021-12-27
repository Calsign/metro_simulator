#[derive(Debug, PartialEq, Eq, Clone, Copy, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Rect {
    pub min_x: u64,
    pub max_x: u64,
    pub min_y: u64,
    pub max_y: u64,
}

impl Rect {
    pub fn xywh(x: u64, y: u64, w: u64, h: u64) -> Self {
        return Self {
            min_x: x,
            max_x: x + w,
            min_y: y,
            max_y: y + h,
        };
    }
}
