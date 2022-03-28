use std::path::PathBuf;

use lazy_static::lazy_static;

use engine::state::State;
use route::{best_route, CarConfig, Graph, Node, QueryInput, Route, WorldState};

#[derive(Debug, Clone)]
pub enum StringPredicate {
    MatchesStr(&'static str),
    MatchesString(String),
    ContainsStr(&'static str),
    ContainsString(String),
}

impl StringPredicate {
    fn matches(&self, test: &str) -> bool {
        use StringPredicate::*;
        match self {
            MatchesStr(s) => &test == s,
            MatchesString(s) => &test == s,
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
    CostInRangeSeconds(f64, f64),
    CostInRangeMinutes(f64, f64),
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
                .any(|n| matches!(n, Node::MetroStop { metro_line, .. } if metro_line == id)),
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
    pub world_state: WorldState,
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
            world_state: WorldState::new(),
            car_config,
        }
    }
}

use RoutePredicate::*;

lazy_static! {
    static ref SFO: Coord = (2109, 2488);
    static ref SF_DOWNTOWN: Coord = (2087, 2008);
    static ref DALY_CITY: Coord = (1924, 2252);
    static ref OAKLAND_DOWNTOWN: Coord = (2370, 1965);
    static ref PITTSBURG: Coord = (3084, 1364);
    static ref PLEASANTON: Coord = (3186, 2246);
    static ref SAN_MATEO: Coord = (2318, 2662);
    static ref STANFORD: Coord = (2590, 2994);
    static ref SUNNYBALUE: Coord = (2893, 3079);

    pub static ref TESTS: Box<[RouteTest]> = Box::new([
        RouteTest::new(
            "sfo -> downtown",
            *SFO,
            *SF_DOWNTOWN,
            vec![
                CostInRangeMinutes(15.0, 40.0),
                HasMetroStation("San Francisco International Airport".into()),
                HasMetroStop("Daly City".into()),
                HasMetroStation("Montgomery Street".into()),
                Not(HasMetroStop(StringPredicate::ContainsStr("Oakland")).into()),
                Or(vec![
                    HasMetroLine(1),
                    HasMetroLine(2),
                    HasMetroLine(6),
                    HasMetroLine(11),
                    HasMetroLine(12)
                ]),
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
    ]);
}

pub fn setup() -> (State, Graph) {
    let state = State::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    let graph = state.construct_base_route_graph().unwrap();
    (state, graph)
}

pub fn perform_query(state: &State, graph: &mut Graph, test: &RouteTest) -> Route {
    let start = state.qtree.get_address(test.start.0, test.start.1).unwrap();
    let end = state.qtree.get_address(test.end.0, test.end.1).unwrap();

    best_route(QueryInput {
        base_graph: graph,
        start,
        end,
        state: &test.world_state,
        car_config: test.car_config.clone(),
    })
    .unwrap()
    .unwrap()
}
