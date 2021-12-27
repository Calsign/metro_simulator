use crate::tile::{Capacities, Tile};

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct WorkplaceTile {
    pub density: usize,
}

#[typetag::serde]
impl Tile for WorkplaceTile {
    fn name(&self) -> &'static str {
        "workplace"
    }

    fn capacities(&self) -> Capacities {
        let mut capacities = Capacities::default();
        capacities.workers = self.density;
        capacities
    }
}
