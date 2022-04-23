use bencher::{benchmark_group, benchmark_main, Bencher};

fn no_car_benchmark(bench: &mut Bencher) {
    let (state, mut graph) = sf_routes::setup();
    bench.iter(|| sf_routes::perform_query(&state, &mut graph, &sf_routes::TESTS[0]));
}

fn with_car_benchmark(bench: &mut Bencher) {
    let (state, mut graph) = sf_routes::setup();
    bench.iter(|| sf_routes::perform_query(&state, &mut graph, &sf_routes::TESTS[4]));
}

benchmark_group!(benches, no_car_benchmark, with_car_benchmark);
benchmark_main!(benches);
