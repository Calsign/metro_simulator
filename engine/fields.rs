use std::collections::HashMap;

use serde::{Deserialize, Serialize};

struct ComputeLeafData<'a, 'b, 'c, 'd> {
    tile: &'a tiles::Tile,
    data: &'b quadtree::VisitData,
    extra: &'c FieldsComputationData<'d>,
}

struct ComputeBranchData<'a, 'b, 'c, 'd> {
    fields: &'a quadtree::QuadMap<FieldsState>,
    data: &'b quadtree::VisitData,
    extra: &'c FieldsComputationData<'d>,
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
    // NOTE: this is stored in Population rather than Employment because it is based on where people
    // live, not where they work
    pub employed_people: usize,
}

impl Population {
    /// the fraction of total housing that is occupied
    pub fn housing_saturation(&self) -> f64 {
        if self.housing.total > 0 {
            self.people.total as f64 / self.housing.total as f64
        } else {
            1.0
        }
    }

    /// the fraction of total housing that is vacant
    pub fn housing_vacancy(&self) -> f64 {
        1.0 - self.housing_saturation()
    }

    /// the total number of empty housing units
    pub fn empty_housing(&self) -> usize {
        self.housing.total - self.people.total
    }

    /// the fraction of people that have jobs
    pub fn employment_rate(&self) -> f64 {
        // NOTE: the unemployment rate is not the opposite of this, it should take into account
        // whether people are not in the workforce for other reasons.
        if self.people.total > 0 {
            self.employed_people as f64 / self.people.total as f64
        } else {
            1.0
        }
    }
}

impl Field for Population {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let (people, housing, employed_people) = match leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) => {
                let employed_people: usize = agents
                    .iter()
                    .filter_map(|id| leaf.extra.agents.get(id).expect("missing agent").workplace)
                    .count();
                (agents.len(), *density, employed_people)
            }
            _ => (0, 0, 0),
        };
        Some(Self {
            people: SimpleDensity::from_total(people, leaf.data),
            housing: SimpleDensity::from_total(housing, leaf.data),
            employed_people,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        use itertools::Itertools;
        let (people, housing, employed_people): (Vec<usize>, Vec<usize>, Vec<usize>) = branch
            .fields
            .values()
            .iter()
            .map(|f| {
                (
                    f.population.people.total,
                    f.population.housing.total,
                    f.population.employed_people,
                )
            })
            .multiunzip();
        Some(Self {
            people: SimpleDensity::from_total(people.iter().sum(), branch.data),
            housing: SimpleDensity::from_total(housing.iter().sum(), branch.data),
            employed_people: employed_people.iter().sum(),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Employment {
    pub workers: SimpleDensity,
    pub jobs: SimpleDensity,
}

impl Employment {
    /// the fraction of total jobs that are occupied
    pub fn job_saturation(&self) -> f64 {
        if self.jobs.total > 0 {
            self.workers.total as f64 / self.jobs.total as f64
        } else {
            1.0
        }
    }

    /// the fraction of jobs that are unfilled
    pub fn job_vacancy(&self) -> f64 {
        1.0 - self.job_saturation()
    }

    /// the total number of unfilled jobs
    pub fn unfilled_jobs(&self) -> usize {
        self.jobs.total - self.workers.total
    }
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

// TODO: it could make sense to split this out of Engine into a separate state, like State
pub struct FieldsComputationData<'a> {
    pub agents: &'a HashMap<u64, agent::Agent>,
}

impl FieldsState {
    pub(crate) fn compute_leaf<'a>(
        &mut self,
        tile: &tiles::Tile,
        data: &quadtree::VisitData,
        extra: &FieldsComputationData<'a>,
    ) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_leaf(ComputeLeafData { tile, data, extra }) {
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

    pub(crate) fn compute_branch<'a>(
        &mut self,
        fields: &quadtree::QuadMap<FieldsState>,
        data: &quadtree::VisitData,
        extra: &FieldsComputationData<'a>,
    ) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_branch(ComputeBranchData {
                    fields,
                    data,
                    extra,
                }) {
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

impl state::Fields for FieldsState {}

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
