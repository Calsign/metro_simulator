use std::collections::HashMap;

use serde::{Deserialize, Serialize};

struct ComputeLeafData<'a, 'b, 'c, 'd, 'e, 'f> {
    tile: &'a tiles::Tile,
    data: &'b quadtree::VisitData,
    extra: &'c FieldsComputationData<'d, 'e>,
    current: &'f FieldsState,
}

struct ComputeBranchData<'a, 'b, 'c, 'd, 'e> {
    fields: &'a quadtree::QuadMap<FieldsState>,
    data: &'b quadtree::VisitData,
    extra: &'c FieldsComputationData<'d, 'e>,
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

    fn add_weighted_sample(&mut self, value: f64, weight: usize) {
        *self = *self
            + Self {
                value,
                count: weight,
            }
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

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct Population {
    pub people: SimpleDensity,
    pub housing: SimpleDensity,
    // NOTE: this is stored in Population rather than Employment because it is based on where people
    // live, not where they work
    pub employed_people: usize,
    pub workplace_happiness: WeightedAverage,
    pub commute_duration: WeightedAverage,
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
        let mut people = 0;
        let mut housing = 0;
        let mut employed_people = 0;
        let mut workplace_happiness = WeightedAverage::zero();
        let mut commute_duration = WeightedAverage::zero();

        if let tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) = leaf.tile {
            people = agents.len();
            housing = *density;
            for agent_id in agents {
                let agent = leaf.extra.agents.get(agent_id).expect("missing agent");
                if let Some(workplace) = agent.workplace {
                    employed_people += 1;
                    workplace_happiness
                        .add_sample(agent.workplace_happiness_score().unwrap() as f64);
                    commute_duration.add_sample(agent.average_commute_length() as f64);
                }
            }
        }

        Some(Self {
            people: SimpleDensity::from_total(people, leaf.data),
            housing: SimpleDensity::from_total(housing, leaf.data),
            employed_people,
            workplace_happiness,
            commute_duration,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.population),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct Employment {
    pub workers: SimpleDensity,
    pub jobs: SimpleDensity,
    pub workplace_happiness: WeightedAverage,
    pub commute_duration: WeightedAverage,
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
        let mut workers = 0;
        let mut jobs = 0;
        let mut workplace_happiness = WeightedAverage::zero();
        let mut commute_duration = WeightedAverage::zero();

        if let tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }) = leaf.tile {
            workers = agents.len();
            jobs = *density;
            for agent_id in agents {
                let agent = leaf.extra.agents.get(agent_id).expect("missing agent");
                workplace_happiness.add_sample(agent.workplace_happiness_score().unwrap() as f64);
                commute_duration.add_sample(agent.average_commute_length() as f64);
            }
        }

        Some(Self {
            workers: SimpleDensity::from_total(workers, leaf.data),
            jobs: SimpleDensity::from_total(jobs, leaf.data),
            workplace_happiness,
            commute_duration,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.employment),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct RawLandValue {
    /// total density of constructed tiles
    pub combined_density: SimpleDensity,
    pub raw_land_value: WeightedAverage,
    pub raw_construction_cost: WeightedAverage,
}

impl Field for RawLandValue {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let mut raw_land_value = WeightedAverage::zero();
        let mut raw_construction_cost = WeightedAverage::zero();

        let combined_density = leaf.current.population.housing + leaf.current.employment.jobs;
        let area = leaf.data.area(leaf.extra.config.min_tile_size) as usize;

        match leaf.tile {
            tiles::Tile::EmptyTile(_) => {
                // land has an inherent value
                raw_land_value.add_weighted_sample(1.0, area);
                // TODO: account for terrain
                raw_construction_cost.add_weighted_sample(1.0, area); // 1x
            }
            tiles::Tile::WaterTile(_) => {
                // construction cost drops off a lot faster than land value, so the result should be
                // very rare construction on water, but high land values and reasonable construction
                // costs near water

                // water has a high base land value
                raw_land_value.add_weighted_sample(10.0, area);
                // high cost to building on water
                raw_construction_cost.add_weighted_sample(100.0, area); // 100x
            }
            tiles::Tile::MetroStationTile(_) => {
                // metro stations have high land value
                raw_land_value.add_weighted_sample(1000.0, area);
                // it's difficult to build near metro stations
                raw_construction_cost.add_weighted_sample(10.0, area); // 10x
            }
            _ => {
                // TODO: should really depend on the *types* of housing and jobs that are here
                raw_land_value.add_weighted_sample(combined_density.density() * 1000.0, area);
                raw_construction_cost
                    .add_weighted_sample((combined_density.density() * 200.0).max(1.0), area);
            }
        }

        // this is a cost multiplier
        assert!(raw_construction_cost.value >= 1.0);

        Some(Self {
            combined_density,
            raw_land_value,
            raw_construction_cost,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.raw_land_value),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct RawDemand {
    pub raw_workplace_demand: WeightedAverage,
}

impl Field for RawDemand {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let mut raw_workplace_demand = WeightedAverage::zero();

        if leaf.current.employment.jobs.total > 0 && leaf.current.employment.job_saturation() >= 0.9
        {
            // if a workplace is nearly fully-staffed, they open more positions
            raw_workplace_demand.add_weighted_sample(leaf.current.employment.jobs.total as f64, 1);
        }

        Some(Self {
            raw_workplace_demand,
        })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.raw_demand),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct LandValue {
    /// average value of land, per tile, in dollars
    pub land_value: WeightedAverage,
    /// average cost multiplier for performing construction here
    pub construction_cost: WeightedAverage,
}

impl Field for LandValue {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        Some(leaf.current.land_value)
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(
            branch.fields.values().iter().map(|f| f.land_value),
        ))
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::Add)]
pub struct Demand {
    pub workplace_demand: WeightedAverage,
}

impl Field for Demand {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        Some(leaf.current.demand)
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(sum_iter(branch.fields.values().iter().map(|f| f.demand)))
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum FieldPass {
    First,
    Second,
}

// TODO: write a procedural macro to make this less painful
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub struct FieldsState {
    pub population: Population,
    pub employment: Employment,
    pub raw_land_value: RawLandValue,
    pub raw_demand: RawDemand,
    pub land_value: LandValue,
    pub demand: Demand,
}

// TODO: it could make sense to split this out of Engine into a separate state, like State
pub struct FieldsComputationData<'a, 'b> {
    pub config: &'a state::Config,
    pub agents: &'b HashMap<u64, agent::Agent>,
}

impl FieldsState {
    pub(crate) fn compute_leaf<'a, 'b>(
        &mut self,
        tile: &tiles::Tile,
        data: &quadtree::VisitData,
        extra: &FieldsComputationData<'a, 'b>,
        pass: FieldPass,
    ) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_leaf(ComputeLeafData {
                    tile,
                    data,
                    extra,
                    current: self,
                }) {
                    Some(val) => {
                        self.$name = val;
                        changed = true;
                    }
                    None => (),
                }
            }};
        }

        match pass {
            FieldPass::First => {
                each_field!(Population, population);
                each_field!(Employment, employment);
                each_field!(RawLandValue, raw_land_value);
                each_field!(RawDemand, raw_demand);
            }
            FieldPass::Second => {
                each_field!(LandValue, land_value);
                each_field!(Demand, demand);
            }
        }

        changed
    }

    pub(crate) fn compute_branch<'a, 'b>(
        &mut self,
        fields: &quadtree::QuadMap<FieldsState>,
        data: &quadtree::VisitData,
        extra: &FieldsComputationData<'a, 'b>,
        pass: FieldPass,
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

        match pass {
            FieldPass::First => {
                each_field!(Population, population);
                each_field!(Employment, employment);
                each_field!(RawLandValue, raw_land_value);
                each_field!(RawDemand, raw_demand);
            }
            FieldPass::Second => {
                each_field!(LandValue, land_value);
                each_field!(Demand, demand);
            }
        }

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
