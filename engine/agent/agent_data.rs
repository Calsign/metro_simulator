use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EducationDegree {
    NoDegree,
    HighSchool,
    Undergrad,
    Masters,
    Phd,
}

impl EducationDegree {
    pub fn from_years_of_education(years_of_education: u32) -> Self {
        // this is very US-centric
        if years_of_education >= 20 {
            Self::Phd
        } else if years_of_education >= 17 {
            Self::Masters
        } else if years_of_education >= 16 {
            Self::Undergrad
        } else if years_of_education >= 12 {
            Self::HighSchool
        } else {
            Self::NoDegree
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::NoDegree => "No degree",
            Self::HighSchool => "High school",
            Self::Undergrad => "Undergrad",
            Self::Masters => "Masters",
            Self::Phd => "PhD",
        }
    }
}

impl std::fmt::Display for EducationDegree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Age(u32);

// this is very US-centric
impl Age {
    pub fn years(&self) -> u32 {
        self.0
    }

    pub fn is_adult(&self) -> bool {
        self.0 >= 18
    }

    pub fn is_senior(&self) -> bool {
        self.0 >= 65
    }

    pub fn is_working_age(&self) -> bool {
        self.0 >= 15 && self.0 < 65
    }
}

impl std::fmt::Display for Age {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentData {
    /// used to compute age
    pub birthday: chrono::NaiveDate,
    /// total years of schooling
    pub years_of_education: u32,
}

impl AgentData {
    pub fn age(&self, current_time: chrono::NaiveDate) -> Age {
        use chrono::Datelike;
        use std::cmp::Ordering;

        // turns out dealing with date differences is kind of tricky.
        let diff = (current_time.year() - self.birthday.year()) as u32;
        let minus_one = match current_time.month().cmp(&self.birthday.month()) {
            Ordering::Less => true,
            Ordering::Equal => current_time.day() < self.birthday.day(),
            Ordering::Greater => false,
        };
        Age(if minus_one { diff - 1 } else { diff })
    }

    pub fn education_degree(&self) -> EducationDegree {
        EducationDegree::from_years_of_education(self.years_of_education)
    }

    /// How much this agent likes to stay in the same housing situation.
    /// 1.0 means they never move; 0.0 means they constantly want to move.
    pub fn housing_stickiness(&self) -> f32 {
        0.7
    }

    /// How much this agent likes to stay at the same job.
    /// 1.0 means they never leave their job; 0.0 means they constantly look for new jobs.
    pub fn workplace_stickiness(&self) -> f32 {
        0.5
    }

    /// How long this agent is willing to drive to/from work (each way), in seconds.
    pub fn commute_length_tolerance(&self) -> u32 {
        60 * 60 // 1 hour
    }

    pub fn expected_workplace_happiness(&self, commute_length: f32) -> f32 {
        // TODO: a more nuanced approximation here; also, should incorporate other information
        let fraction = commute_length / self.commute_length_tolerance() as f32;
        assert!(fraction >= 0.0);
        1.0 - fraction.min(1.0)
    }
}

#[cfg(test)]
mod agent_data_tests {
    use crate::agent_data::*;

    fn with_birthday(year: i32, month: u32, day: u32) -> AgentData {
        AgentData {
            birthday: chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap(),
            years_of_education: 0,
        }
    }

    #[test]
    fn age() {
        use chrono::NaiveDate;

        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2000, 2, 15).unwrap()),
            Age(0)
        );
        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2000, 10, 1).unwrap()),
            Age(0),
        );
        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2001, 2, 14).unwrap()),
            Age(0),
        );
        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2001, 2, 15).unwrap()),
            Age(1),
        );
        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2001, 10, 1).unwrap()),
            Age(1),
        );
        assert_eq!(
            with_birthday(2000, 2, 15).age(NaiveDate::from_ymd_opt(2010, 2, 15).unwrap()),
            Age(10),
        );
    }

    #[test]
    fn education_degree() {
        assert!(EducationDegree::NoDegree < EducationDegree::HighSchool);
        assert!(EducationDegree::HighSchool < EducationDegree::Undergrad);
        assert!(EducationDegree::Undergrad < EducationDegree::Masters);
        assert!(EducationDegree::Masters < EducationDegree::Phd);
    }
}
