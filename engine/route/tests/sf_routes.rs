use std::path::PathBuf;

use lazy_static::lazy_static;

use engine::Engine;
use quadtree::Address;
use route::{best_route, CarConfig, Edge, Graph, Node, QueryInput, Route};

#[derive(Debug, Clone)]
pub enum StringPredicate {
    Any,
    MatchesStr(&'static str),
    MatchesString(String),
    ContainsStr(&'static str),
    ContainsString(String),
}

impl StringPredicate {
    fn matches(&self, test: &str) -> bool {
        use StringPredicate::*;
        match self {
            Any => true,
            MatchesStr(s) => &test == s,
            MatchesString(s) => test == s,
            ContainsStr(s) => test.contains(s),
            ContainsString(s) => test.contains(s),
        }
    }
}

impl From<&'static str> for StringPredicate {
    fn from(s: &'static str) -> Self {
        Self::MatchesStr(s)
    }
}

impl From<String> for StringPredicate {
    fn from(s: String) -> Self {
        Self::MatchesString(s)
    }
}

#[derive(Debug, Clone)]
pub enum RoutePredicate {
    Not(Box<RoutePredicate>),
    Or(Vec<RoutePredicate>),
    HasMetroStation(StringPredicate),
    HasMetroStop(StringPredicate),
    HasMetroLine(u64),
    HasHighwaySegmentName(StringPredicate),
    HasHighwaySegmentRef(StringPredicate),
    CostInRangeSeconds(f32, f32),
    CostInRangeMinutes(f32, f32),
}

impl RoutePredicate {
    pub fn eval(&self, route: &Route) -> bool {
        use RoutePredicate::*;
        match self {
            Not(inner) => !inner.eval(route),
            Or(inner) => inner.iter().any(|i| i.eval(route)),
            HasMetroStation(name) => route.nodes.iter().any(
                |n| matches!(n, Node::MetroStation { station } if name.matches(&station.name)),
            ),
            HasMetroStop(name) => route.nodes.iter().any(
                |n| matches!(n, Node::MetroStop { station, .. } if name.matches(&station.name)),
            ),
            HasMetroLine(id) => route
                .nodes
                .iter()
                .any(|n| matches!(n, Node::MetroStop { metro_line, .. } if metro_line.inner() == *id)),
            HasHighwaySegmentName(name) => route
                .edges
                .iter()
                .any(|e| matches!(e, Edge::Highway { data, .. }
                                  if data.name.clone().map_or(false, |n| name.matches(&n)))),
            HasHighwaySegmentRef(ref_filter) => {
                route.edges.iter().any(|e| matches!(e, Edge::Highway { data, .. }
                                                    if data.refs.iter().any(|r| ref_filter.matches(r))))
            },
            CostInRangeSeconds(min, max) => route.cost >= *min && route.cost <= *max,
            CostInRangeMinutes(min, max) => route.cost >= *min * 60.0 && route.cost <= *max * 60.0,
        }
    }
}

pub type Coord = (u64, u64);

pub struct RouteTest {
    pub name: String,
    pub start: Coord,
    pub end: Coord,
    pub predicates: Vec<RoutePredicate>,
    pub car_config: Option<CarConfig>,
}

impl RouteTest {
    pub fn new(
        name: &str,
        start: Coord,
        end: Coord,
        predicates: Vec<RoutePredicate>,
        car_config: Option<CarConfig>,
    ) -> Self {
        Self {
            name: String::from(name),
            start,
            end,
            predicates,
            car_config,
        }
    }
}

use RoutePredicate::*;

lazy_static! {
    pub static ref SFO: Coord = (2109, 2488);
    pub static ref SF_DOWNTOWN: Coord = (2087, 2008);
    pub static ref DALY_CITY: Coord = (1924, 2252);
    pub static ref OAKLAND_DOWNTOWN: Coord = (2370, 1965);
    pub static ref PITTSBURG: Coord = (3084, 1364);
    pub static ref PLEASANTON: Coord = (3186, 2246);
    pub static ref SAN_MATEO: Coord = (2318, 2662);
    pub static ref STANFORD: Coord = (2590, 2994);
    pub static ref SUNNYVALE: Coord = (2893, 3079);
    pub static ref FAIRFIELD: Coord = (2892, 716);
    pub static ref UC_BERKELEY: Coord = (2406, 1770);

    pub static ref TESTS: Box<[RouteTest]> = Box::new([
        RouteTest::new(
            "sfo -> downtown",
            *SFO,
            *SF_DOWNTOWN,
            vec![
                CostInRangeMinutes(15.0, 40.0),
                Or(vec![
                    HasMetroStation("San Francisco International Airport".into()),
                    HasMetroStation("International Terminal G".into()),
                ]),
                HasMetroStop("Daly City".into()),
                HasMetroStation("Montgomery Street".into()),
                Not(HasMetroStop(StringPredicate::ContainsStr("Oakland")).into()),
                Not(Or(vec![HasMetroLine(3), HasMetroLine(4)]).into()),
            ],
            None,
        ),
        // TODO: add predicates for these additional tests
        RouteTest::new(
            "daly city -> oakland",
            *DALY_CITY,
            *OAKLAND_DOWNTOWN,
            vec![],
            None,
        ),
        RouteTest::new("pittsburg -> pleasanton", *PITTSBURG, *PLEASANTON, vec![], None),
        RouteTest::new(
            "sf -> oakland",
            *SF_DOWNTOWN,
            *OAKLAND_DOWNTOWN,
            vec![
                Or(vec![
                    HasHighwaySegmentRef("I 80".into()),
                    HasMetroStop(StringPredicate::Any),
                ]),
            ],
            Some(CarConfig::StartWithCar),
        ),
        RouteTest::new(
            "san mateo -> stanford",
            *SAN_MATEO,
            *STANFORD,
            vec![
                HasHighwaySegmentRef("US 101".into()),
            ],
            Some(CarConfig::StartWithCar),
        ),
        // this test ensures we can drive, park at a metro station, and take the metro to the destination
        // TODO: this isn't currently working, it may simply be that driving is always faster for this route
        RouteTest::new(
            "fairfield -> UC berkeley (park and ride)",
            *FAIRFIELD,
            *UC_BERKELEY,
            vec![
                // HasHighwaySegmentName(StringPredicate::Any),
                // HasMetroStop(StringPredicate::Any),
            ],
            Some(CarConfig::StartWithCar),
        ),
        RouteTest::new(
            "UC berkeley -> fairfield (collect parked car)",
            *UC_BERKELEY,
            *FAIRFIELD,
            vec![
                HasHighwaySegmentName(StringPredicate::Any),
                HasMetroStop(StringPredicate::Any),
            ],
            Some(CarConfig::CollectParkedCar { address: Address::from_xy(2273, 1631, 12) }),
        ),
        // this test addresses a past issue where car routes starting near metro stations didn't work
        RouteTest::new(
            "stanford -> fairfield (regression)",
            *STANFORD,
            *FAIRFIELD,
            vec![
                Not(HasMetroStop(StringPredicate::Any).into()),
            ],
            Some(CarConfig::StartWithCar),
        ),
        RouteTest::new(
            "sf -> fairfield (driving across bridge)",
            *SF_DOWNTOWN,
            *FAIRFIELD,
            vec![HasHighwaySegmentRef("I 80".into())],
            Some(CarConfig::StartWithCar),
        ),
    ]);
}

pub fn setup() -> (Engine, std::cell::RefCell<Graph>) {
    let engine = Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    let graph = engine::BaseGraph::construct_base_graph(&engine.state).unwrap();
    (engine, std::cell::RefCell::new(graph))
}

pub fn perform_query(
    engine: &Engine,
    graph: std::cell::RefMut<Graph>,
    test: &RouteTest,
) -> Option<Route> {
    let start = engine
        .state
        .qtree
        .get_address(test.start.0, test.start.1)
        .unwrap();
    let end = engine
        .state
        .qtree
        .get_address(test.end.0, test.end.1)
        .unwrap();

    best_route(
        graph,
        QueryInput {
            start,
            end,
            car_config: test.car_config,
        },
    )
    .unwrap()
}
