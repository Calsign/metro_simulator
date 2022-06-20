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

fn sum_iter<T, I>(iterator: I) -> T
where
    T: std::ops::Add<Output = T> + Default + Copy,
    I: IntoIterator<Item = T>,
{
    use std::borrow::Borrow;
    iterator.into_iter().fold(T::default(), std::ops::Add::add)
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct SimpleDensity {
    pub total: usize,
    pub area: u64,
}

impl SimpleDensity {
    fn from_total(total: usize, data: &quadtree::VisitData) -> Self {
        Self {
            total,
            area: data.width.pow(2),
        }
    }

    pub fn density(&self) -> f64 {
        self.total as f64 / self.area as f64
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct WeightedAverage {
    pub value: f64,
    pub count: usize,
}

impl WeightedAverage {
    fn zero() -> Self {
        Self {
            value: 0.0,
            count: 0,
        }
    }

    fn one(value: f64) -> Self {
        Self { value, count: 1 }
    }

    fn add_sample(&mut self, value: f64) {
        *self = *self + Self::one(value)
    }
}

impl std::ops::Add for WeightedAverage {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let total = self.count + other.count;
        let (self_share, other_share) = if total > 0 {
            (
                self.count as f64 / total as f64,
                other.count as f64 / total as f64,
            )
        } else {
            (0.0, 0.0)
        };
        Self {
            value: self.value as f64 * self_share + other.value as f64 * other_share,
            count: total,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, derive_more::Add)]
pub struct Population {
    pub people: SimpleDensity,
    pub housing: SimpleDensity,
    // NOTE: this is stored in Population rather than Employment because it is based on where people
    // live, not where they work
    pub employed_people: usize,
    pub workplace_happiness: WeightedAverage,
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
        let (people, housing, employed_people, workplace_happiness) = match leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) => {
                let mut employed_people = 0;
                let mut workplace_happiness = WeightedAverage::zero();
                for agent_id in agents {
                    let agent = leaf.extra.agents.get(agent_id).expect("missing agent");
                    if let Some(workplace) = agent.workplace {
                        employed_people += 1;
                        workplace_happiness
                            .add_sample(agent.workplace_happiness_score().unwrap() as f64);
                    }
                }
                (agents.len(), *density, employed_people, workplace_happiness)
            }
            _ => (0, 0, 0, WeightedAverage::zero()),
        };
        Some(Self {
            people: SimpleDensity::from_total(people, leaf.data),
            housing: SimpleDensity::from_total(housing, leaf.data),
            employed_people,
            workplace_happiness,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.population),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, derive_more::Add)]
pub struct Employment {
    pub workers: SimpleDensity,
    pub jobs: SimpleDensity,
    pub workplace_happiness: WeightedAverage,
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
        let (workers, jobs, workplace_happiness) = match leaf.tile {
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }) => {
                let mut workplace_happiness = WeightedAverage::zero();
                for agent_id in agents {
                    let agent = leaf.extra.agents.get(agent_id).expect("missing agent");
                    workplace_happiness
                        .add_sample(agent.workplace_happiness_score().unwrap() as f64);
                }
                (agents.len(), *density, workplace_happiness)
            }
            _ => (0, 0, WeightedAverage::zero()),
        };
        Some(Self {
            workers: SimpleDensity::from_total(workers, leaf.data),
            jobs: SimpleDensity::from_total(jobs, leaf.data),
            workplace_happiness,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.employment),
        ))
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

#[cfg(test)]
mod tests {
    use crate::fields::*;

    #[test]
    fn simple_density_test() {
        assert_eq!(
            (SimpleDensity {
                total: 100,
                area: 1,
            } + SimpleDensity { total: 4, area: 3 })
            .density(),
            26.0,
        );
    }

    #[test]
    fn weighted_average_test() {
        assert_eq!(
            WeightedAverage {
                value: 1.0,
                count: 10
            } + WeightedAverage {
                value: 4.0,
                count: 5,
            },
            WeightedAverage {
                value: 2.0,
                count: 15,
            }
        );
    }
}
