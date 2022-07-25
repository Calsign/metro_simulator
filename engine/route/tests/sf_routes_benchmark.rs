use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Mutex;

use bencher::{benchmark_main, Bencher, TDynBenchFn, TestDesc, TestDescAndFn, TestFn};
use once_cell::sync::Lazy;

// NOTE: we setup the problem twice because I couldn't figure out how to split the borrow on the
// tuple
static ENGINE: Lazy<Mutex<engine::Engine>> = Lazy::new(|| Mutex::new(sf_routes::setup().0));
static GRAPH: Lazy<Mutex<RefCell<route::Graph>>> = Lazy::new(|| Mutex::new(sf_routes::setup().1));

struct RouteBench {
    test: &'static sf_routes::RouteTest,
}

impl TDynBenchFn for RouteBench {
    fn run(&self, bench: &mut Bencher) {
        bench.iter(|| {
            sf_routes::perform_query(
                &ENGINE.lock().unwrap(),
                GRAPH.lock().unwrap().borrow_mut(),
                self.test,
            )
            .unwrap();
        });
    }
}

fn benches() -> Vec<TestDescAndFn> {
    let mut benches = Vec::new();

    for test in &sf_routes::TESTS[..] {
        benches.push(TestDescAndFn {
            desc: TestDesc {
                name: Cow::from(test.name.clone()),
                ignore: false,
            },
            testfn: TestFn::DynBenchFn(Box::new(RouteBench { test })),
        });
    }

    benches
}

benchmark_main!(benches);
