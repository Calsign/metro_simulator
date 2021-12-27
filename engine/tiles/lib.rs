pub mod housing;
pub mod tile;
pub mod workplace;

pub use crate::tile::{Capacities, Tile};

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct EmptyTile {}

#[typetag::serde]
impl Tile for EmptyTile {
    fn name(&self) -> &'static str {
        "empty"
    }

    fn capacities(&self) -> Capacities {
        Capacities::default()
    }
}
