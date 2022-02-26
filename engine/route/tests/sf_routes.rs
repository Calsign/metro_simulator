use std::path::PathBuf;

use lazy_static::lazy_static;

use engine::state::State;
use route::{best_route, Node, Route, WorldState};

#[derive(Debug, Clone)]
enum StringPredicate {
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
enum RoutePredicate {
    Not(Box<RoutePredicate>),
    Or(Vec<RoutePredicate>),
    HasMetroStation(StringPredicate),
    HasMetroStop(StringPredicate),
    HasMetroLine(u64),
    CostInRangeSeconds(f64, f64),
    CostInRangeMinutes(f64, f64),
}

impl RoutePredicate {
    fn eval(&self, route: &Route) -> bool {
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

type Coord = (u64, u64);

struct RouteTest {
    name: String,
    start: Coord,
    end: Coord,
    predicates: Vec<RoutePredicate>,
    world_state: WorldState,
}

impl RouteTest {
    fn new(name: &str, start: Coord, end: Coord, predicates: Vec<RoutePredicate>) -> Self {
        Self {
            name: String::from(name),
            start,
            end,
            predicates,
            world_state: WorldState::new(),
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
    static ref TESTS: Box<[RouteTest]> = Box::new([
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
            ]
        ),
        // TODO: add predicates for these additional tests
        RouteTest::new(
            "daly city -> oakland",
            *DALY_CITY,
            *OAKLAND_DOWNTOWN,
            vec![]
        ),
        RouteTest::new("pittsburg -> pleasanton", *PITTSBURG, *PLEASANTON, vec![],),
    ]);
}

#[test]
fn sf_routes_test() {
    let state = State::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    let mut graph = state.construct_base_route_graph().unwrap();

    let mut success = true;
    for test in TESTS.iter() {
        println!("Computing best route for {}", test.name);

        let start = state.qtree.get_address(test.start.0, test.start.1).unwrap();
        let end = state.qtree.get_address(test.end.0, test.end.1).unwrap();

        let route = best_route(&mut graph, start, end, &test.world_state)
            .unwrap()
            .unwrap();

        let mut failed_predicates = Vec::new();
        for predicate in &test.predicates {
            if !predicate.eval(&route) {
                println!("Test {} failed predicate {:?}", test.name, predicate);
                failed_predicates.push(predicate.clone());
            }
        }

        if !failed_predicates.is_empty() {
            success = false;

            println!();
            println!(
                "Test {} failed {} predicate(s)",
                test.name,
                failed_predicates.len()
            );
            println!();
            println!(
                "Route with cost {:?} (minutes: {:?}):",
                route.cost,
                route.cost / 60.0,
            );
            for node in route.nodes {
                println!("  {}", node);
            }
            println!();
            println!("Failed predicates:");
            for predicate in failed_predicates {
                println!("  {:?}", predicate);
            }
        }
    }

    println!();
    println!();
    assert!(success, "Some predicates failed");
}
