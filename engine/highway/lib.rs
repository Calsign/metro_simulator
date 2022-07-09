mod highways;
mod junction;
mod segment;
pub mod timing;

pub use highways::Highways;
pub use junction::{HighwayJunction, RampDirection};
pub use segment::{HighwayData, HighwayKey, HighwaySegment, KeyVisitor, SplineVisitor};
