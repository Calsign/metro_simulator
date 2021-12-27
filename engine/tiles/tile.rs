#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Capacities {
    pub residents: usize,
    pub workers: usize,
}

impl Capacities {
    pub fn default() -> Self {
        Self {
            residents: 0,
            workers: 0,
        }
    }
}

#[dyn_clonable::clonable]
#[typetag::serde(tag = "type")]
pub trait Tile: std::fmt::Debug + Clone + Send {
    fn name(&self) -> &'static str;
    fn capacities(&self) -> Capacities;
}
