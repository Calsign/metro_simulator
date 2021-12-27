use crate::tile::{Capacities, Tile};

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct HousingTile {
    pub density: usize,
}

#[typetag::serde]
impl Tile for HousingTile {
    fn name(&self) -> &'static str {
        "housing"
    }

    fn capacities(&self) -> Capacities {
        let mut capacities = Capacities::default();
        capacities.residents = self.density;
        capacities
    }
}
