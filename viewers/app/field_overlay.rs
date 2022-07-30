use uom::si::time::{day, hour};
use uom::si::u64::Time;

lazy_static::lazy_static! {
    static ref COMMUTE_DURATION_MAX_SCALE: f32 = Time::new::<hour>(2).value as f32;
    /// how far back into the past we consider a tile to be "maximum" old
    // TODO: make this horizon longer; short for now for testing purposes
    static ref TILE_CREATION_TIME_HORIZON: i64 = Time::new::<day>(20).value as i64;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, enum_iterator::IntoEnumIterator)]
pub(crate) enum FieldType {
    // population-related
    Population,
    TotalHousing,
    HousingSaturation,
    HousingVacancy,
    EmploymentRate,
    WorkplaceHappinessHome,
    CommuteDurationHome,
    CarOwnership,

    // employment-related
    Employment,
    TotalJobs,
    JobSaturation,
    JobVacancy,
    WorkplaceHappinessWork,
    CommuteDurationWork,

    // land value-related
    TileCreationTimeOldest,
    TileCreationTimeNewest,
    RawLandValue,
    LandValue,
    RawConstructionCost,
    ConstructionCost,

    // demand-related
    RawWorkplaceDemand,
    WorkplaceDemand,

    // dynamic
    Traffic,
    Parking,
}

impl FieldType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Population => "Population",
            Self::TotalHousing => "Total housing",
            Self::HousingSaturation => "Housing saturation",
            Self::HousingVacancy => "Housing vacancy",
            Self::EmploymentRate => "Employment rate",
            Self::WorkplaceHappinessHome => "Workplace happiness (home)",
            Self::CommuteDurationHome => "Commute duration (home)",
            Self::CarOwnership => "Car ownership",

            Self::Employment => "Employment",
            Self::TotalJobs => "Total jobs",
            Self::JobSaturation => "Job saturation",
            Self::JobVacancy => "Job vacancy",
            Self::WorkplaceHappinessWork => "Workplace happiness (work)",
            Self::CommuteDurationWork => "Commute duration (work)",

            Self::TileCreationTimeOldest => "Tile creation time (oldest)",
            Self::TileCreationTimeNewest => "Tile creation time (newest)",
            Self::RawLandValue => "Land value (raw)",
            Self::LandValue => "Land value",
            Self::RawConstructionCost => "Construction cost (raw)",
            Self::ConstructionCost => "Construction cost",

            Self::RawWorkplaceDemand => "Workplace demand (raw)",
            Self::WorkplaceDemand => "Workplace demand",

            Self::Traffic => "Traffic",
            Self::Parking => "Parking",
        }
    }

    fn min(&self, engine: &engine::Engine) -> f32 {
        match self {
            Self::TileCreationTimeOldest | Self::TileCreationTimeNewest => {
                (engine.time_state.current_time as i64 - *TILE_CREATION_TIME_HORIZON) as f32
            }
            _ => 0.0,
        }
    }

    fn max(&self, engine: &engine::Engine) -> f32 {
        match self {
            Self::Population => 0.3,
            Self::TotalHousing => 0.3,
            Self::HousingSaturation => 1.0,
            Self::HousingVacancy => 0.5,
            Self::EmploymentRate => 1.0,
            Self::WorkplaceHappinessHome => 1.0,
            Self::CommuteDurationHome => *COMMUTE_DURATION_MAX_SCALE,
            Self::CarOwnership => 1.0,

            Self::Employment => 0.3,
            Self::TotalJobs => 0.3,
            Self::JobSaturation => 1.0,
            Self::JobVacancy => 0.5,
            Self::WorkplaceHappinessWork => 1.0,
            Self::CommuteDurationWork => *COMMUTE_DURATION_MAX_SCALE,

            Self::TileCreationTimeOldest | Self::TileCreationTimeNewest => {
                engine.time_state.current_time as f32
            }
            Self::RawLandValue | Self::LandValue => 60.0,
            Self::RawConstructionCost | Self::ConstructionCost => 20.0,

            Self::RawWorkplaceDemand | Self::WorkplaceDemand => 4.0,

            Self::Traffic => 0.0,
            Self::Parking => 40.0,
        }
    }

    fn value(
        &self,
        engine: &engine::Engine,
        fields: &engine::FieldsState,
        data: &quadtree::VisitData,
    ) -> f32 {
        match self {
            Self::Population => fields.population.people.density() as f32,
            Self::TotalHousing => fields.population.housing.density() as f32,
            Self::HousingSaturation => fields.population.housing_saturation() as f32,
            Self::HousingVacancy => fields.population.housing_vacancy() as f32,
            Self::EmploymentRate => fields.population.employment_rate() as f32,
            Self::WorkplaceHappinessHome => fields.population.workplace_happiness.value as f32,
            Self::CommuteDurationHome => fields.population.commute_duration.value as f32,
            Self::CarOwnership => fields.population.car_ownership.value as f32,

            Self::Employment => fields.employment.workers.density() as f32,
            Self::TotalJobs => fields.employment.jobs.density() as f32,
            Self::JobSaturation => fields.employment.job_saturation() as f32,
            Self::JobVacancy => fields.employment.job_vacancy() as f32,
            Self::WorkplaceHappinessWork => fields.employment.workplace_happiness.value as f32,
            Self::CommuteDurationWork => fields.employment.commute_duration.value as f32,

            Self::TileCreationTimeOldest => fields
                .raw_land_value
                .tile_creation_time
                .min
                .unwrap_or(i64::MIN) as f32,
            Self::TileCreationTimeNewest => fields
                .raw_land_value
                .tile_creation_time
                .max
                .unwrap_or(i64::MIN) as f32,
            Self::RawLandValue => fields.raw_land_value.raw_land_value.value as f32,
            Self::LandValue => fields.land_value.land_value.value as f32,
            Self::RawConstructionCost => fields.raw_land_value.raw_construction_cost.value as f32,
            Self::ConstructionCost => fields.land_value.construction_cost.value as f32,

            Self::RawWorkplaceDemand => fields.raw_demand.raw_workplace_demand.value as f32,
            Self::WorkplaceDemand => fields.demand.workplace_demand.value as f32,

            Self::Traffic => {
                use route::WorldState;
                let travelers = engine
                    .world_state
                    .get_local_road_zone_travelers(data.x, data.y);
                route::local_traffic::congested_travel_factor(&engine.state.config, travelers)
                    as f32
            }
            Self::Parking => {
                use route::WorldState;
                engine.world_state.get_parking(data.x as f64, data.y as f64) as f32
            }
        }
    }

    pub fn hue(
        &self,
        engine: &engine::Engine,
        fields: &engine::FieldsState,
        data: &quadtree::VisitData,
    ) -> f32 {
        match self {
            Self::Traffic => traffic_hue(self.value(engine, fields, data) as f64),
            _ => {
                let max = self.max(engine);
                let min = self.min(engine);
                calc_hue(self.value(engine, fields, data), min, max)
            }
        }
    }
}

pub fn calc_hue(val: f32, min: f32, max: f32) -> f32 {
    if max > min {
        // ranges from 0.0 (reddish) to 0.5 (blueish)
        (f32::min(f32::max(val, min), max) - min) / (max - min) * 0.5
    } else {
        0.0
    }
}

pub fn traffic_hue(traffic_factor: f64) -> f32 {
    let scaled = (traffic_factor - 1.0).min(5.0) / 5.0;
    (1.0 / 3.0 - (scaled / 3.0)) as f32
}
