use serde::{Deserialize, Serialize};

struct ComputeLeafData<'a, 'b> {
    tile: &'a tiles::Tile,
    data: &'b quadtree::VisitData,
}

struct ComputeBranchData<'a, 'b> {
    fields: &'a quadtree::QuadMap<FieldsState>,
    data: &'b quadtree::VisitData,
}

trait Field: std::fmt::Debug + Default + Clone {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self>;

    fn compute_branch(branch: ComputeBranchData) -> Option<Self>;
}

#[derive(Debug, Default, Clone)]
pub struct SimpleDensity {
    pub total: usize,
    pub density: f64,
}

impl SimpleDensity {
    fn from_total(total: usize, data: &quadtree::VisitData) -> Self {
        let density = total as f64 / (data.width * data.width) as f64;
        Self { total, density }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Population {
    pub people: SimpleDensity,
    pub housing: SimpleDensity,
}

impl Population {
    pub fn housing_occupancy(&self) -> f64 {
        self.people.total as f64 / self.housing.total as f64
    }

    pub fn housing_vacancy(&self) -> f64 {
        1.0 - self.housing_occupancy()
    }
}

impl Field for Population {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let (people, housing) = match leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) => {
                (agents.len(), *density)
            }
            _ => (0, 0),
        };
        Some(Self {
            people: SimpleDensity::from_total(people, leaf.data),
            housing: SimpleDensity::from_total(housing, leaf.data),
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        let (people, housing): (Vec<usize>, Vec<usize>) = branch
            .fields
            .values()
            .iter()
            .map(|f| (f.population.people.total, f.population.housing.total))
            .unzip();
        Some(Self {
            people: SimpleDensity::from_total(people.iter().sum(), branch.data),
            housing: SimpleDensity::from_total(housing.iter().sum(), branch.data),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Employment {
    pub workers: SimpleDensity,
    pub jobs: SimpleDensity,
}

impl Field for Employment {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let (workers, jobs) = match leaf.tile {
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }) => {
                (agents.len(), *density)
            }
            _ => (0, 0),
        };
        Some(Self {
            workers: SimpleDensity::from_total(workers, leaf.data),
            jobs: SimpleDensity::from_total(jobs, leaf.data),
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        let (workers, jobs): (Vec<usize>, Vec<usize>) = branch
            .fields
            .values()
            .iter()
            .map(|f| (f.employment.workers.total, f.employment.jobs.total))
            .unzip();
        Some(Self {
            workers: SimpleDensity::from_total(workers.iter().sum(), branch.data),
            jobs: SimpleDensity::from_total(jobs.iter().sum(), branch.data),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct LandValue {
    pub value: f64,
}

impl Field for LandValue {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        Some(Self { value: 0.0 })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(Self { value: 0.0 })
    }
}

// TODO: write a procedural macro to make this less painful
#[derive(Debug, Default, Clone)]
pub struct FieldsState {
    pub population: Population,
    pub employment: Employment,
    pub land_value: LandValue,
}

impl state::Fields for FieldsState {
    fn compute_leaf(&mut self, tile: &tiles::Tile, data: &quadtree::VisitData) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_leaf(ComputeLeafData { tile, data }) {
                    Some(val) => {
                        self.$name = val;
                        changed = true;
                    }
                    None => (),
                }
            }};
        }

        each_field!(Population, population);
        each_field!(Employment, employment);
        each_field!(LandValue, land_value);

        changed
    }

    fn compute_branch(
        &mut self,
        fields: &quadtree::QuadMap<FieldsState>,
        data: &quadtree::VisitData,
    ) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_branch(ComputeBranchData { fields, data }) {
                    Some(val) => {
                        self.$name = val;
                        changed = true;
                    }
                    None => (),
                }
            }};
        }

        each_field!(Population, population);
        each_field!(Employment, employment);
        each_field!(LandValue, land_value);

        changed
    }
}

// NOTE: Dummy serde implementation, cannot actually be used. We never intend to serialize this
// since it can always be re-computed from the state, but some limitation of the trait bounds system
// in conjunction with the way we are using this in a generic bound in Engine makes the type-checker
// fail if this trait isn't implemented. So here we are.

impl Serialize for FieldsState {
    fn serialize<S: serde::ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        panic!("FieldsState is not deserializable")
    }
}

impl<'de> Deserialize<'de> for FieldsState {
    fn deserialize<D: serde::de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        panic!("FieldsState is not deserializable")
    }
}
