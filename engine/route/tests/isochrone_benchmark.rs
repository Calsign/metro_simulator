use std::sync::Mutex;

use bencher::{benchmark_group, benchmark_main, Bencher};
use once_cell::sync::Lazy;

static ENGINE: Lazy<Mutex<engine::Engine>> = Lazy::new(|| Mutex::new(sf_routes::setup().0));

fn benchmark(bench: &mut Bencher, (x, y): sf_routes::Coord, mode: route::Mode) {
    bench.iter(|| {
        let engine = &ENGINE.lock().unwrap();
        let address = engine.state.qtree.get_address(x, y).unwrap();
        engine.query_isochrone_map(address, mode).unwrap();
    });
}

fn walking_benchmark(bench: &mut Bencher) {
    benchmark(bench, *sf_routes::SF_DOWNTOWN, route::Mode::Walking);
}

fn driving_benchmark(bench: &mut Bencher) {
    benchmark(bench, *sf_routes::SF_DOWNTOWN, route::Mode::Driving);
}

benchmark_group!(benches, walking_benchmark, driving_benchmark);
benchmark_main!(benches);
