use std::cell::RefCell;
use std::sync::Mutex;

use bencher::{benchmark_group, benchmark_main, Bencher};
use once_cell::sync::Lazy;

// NOTE: we setup the problem twice because I couldn't figure out how to split the borrow on the
// tuple
static ENGINE: Lazy<Mutex<engine::Engine>> = Lazy::new(|| Mutex::new(sf_routes::setup().0));
static GRAPH: Lazy<Mutex<RefCell<route::Graph>>> = Lazy::new(|| Mutex::new(sf_routes::setup().1));

fn no_car_benchmark(bench: &mut Bencher) {
    bench.iter(|| {
        sf_routes::perform_query(
            &ENGINE.lock().unwrap(),
            GRAPH.lock().unwrap().borrow_mut(),
            &sf_routes::TESTS[0],
        )
    });
}

fn with_car_benchmark(bench: &mut Bencher) {
    bench.iter(|| {
        sf_routes::perform_query(
            &ENGINE.lock().unwrap(),
            GRAPH.lock().unwrap().borrow_mut(),
            &sf_routes::TESTS[3],
        )
    });
}

benchmark_group!(benches, no_car_benchmark, with_car_benchmark);
benchmark_main!(benches);
