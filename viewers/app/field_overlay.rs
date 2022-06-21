use uom::si::time::hour;
use uom::si::u64::Time;

lazy_static::lazy_static! {
    static ref COMMUTE_DURATION_MAX_SCALE: f32 = Time::new::<hour>(2).value as f32;
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

    // employment-related
    Employment,
    TotalJobs,
    JobSaturation,
    JobVacancy,
    WorkplaceHappinessWork,
    CommuteDurationWork,

    // land value-related
    RawLandValue,
    RawConstructionCost,
    LandValue,
    ConstructionCost,
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

            Self::Employment => "Employment",
            Self::TotalJobs => "Total jobs",
            Self::JobSaturation => "Job saturation",
            Self::JobVacancy => "Job vacancy",
            Self::WorkplaceHappinessWork => "Workplace happiness (work)",
            Self::CommuteDurationWork => "Commute duration (work)",

            Self::RawLandValue => "Land value (raw)",
            Self::RawConstructionCost => "Construction cost (raw)",
            Self::LandValue => "Land value",
            Self::ConstructionCost => "Construction cost",
        }
    }

    fn peak(&self) -> f32 {
        match self {
            Self::Population => 0.3,
            Self::TotalHousing => 0.3,
            Self::HousingSaturation => 1.0,
            Self::HousingVacancy => 0.5,
            Self::EmploymentRate => 1.0,
            Self::WorkplaceHappinessHome => 1.0,
            Self::CommuteDurationHome => *COMMUTE_DURATION_MAX_SCALE,

            Self::Employment => 0.3,
            Self::TotalJobs => 0.3,
            Self::JobSaturation => 1.0,
            Self::JobVacancy => 0.5,
            Self::WorkplaceHappinessWork => 1.0,
            Self::CommuteDurationWork => *COMMUTE_DURATION_MAX_SCALE,

            Self::RawLandValue | Self::LandValue => 60.0,
            Self::RawConstructionCost | Self::ConstructionCost => 20.0,
        }
    }

    fn value(&self, fields: &engine::FieldsState, data: &quadtree::VisitData) -> f32 {
        match self {
            Self::Population => fields.population.people.density() as f32,
            Self::TotalHousing => fields.population.housing.density() as f32,
            Self::HousingSaturation => fields.population.housing_saturation() as f32,
            Self::HousingVacancy => fields.population.housing_vacancy() as f32,
            Self::EmploymentRate => fields.population.employment_rate() as f32,
            Self::WorkplaceHappinessHome => fields.population.workplace_happiness.value as f32,
            Self::CommuteDurationHome => fields.population.commute_duration.value as f32,

            Self::Employment => fields.employment.workers.density() as f32,
            Self::TotalJobs => fields.employment.jobs.density() as f32,
            Self::JobSaturation => fields.employment.job_saturation() as f32,
            Self::JobVacancy => fields.employment.job_vacancy() as f32,
            Self::WorkplaceHappinessWork => fields.employment.workplace_happiness.value as f32,
            Self::CommuteDurationWork => fields.employment.commute_duration.value as f32,

            Self::RawLandValue => fields.raw_land_value.raw_land_value.value as f32,
            Self::RawConstructionCost => fields.raw_land_value.raw_construction_cost.value as f32,
            Self::LandValue => fields.land_value.land_value.value as f32,
            Self::ConstructionCost => fields.land_value.construction_cost.value as f32,
        }
    }

    pub fn hue(&self, fields: &engine::FieldsState, data: &quadtree::VisitData) -> f32 {
        if self.peak() > 0.0 {
            // ranges from 0.0 (reddish) to 0.5 (blueish)
            f32::min(self.value(fields, data), self.peak()) / self.peak() * 0.5
        } else {
            0.0
        }
    }
}
