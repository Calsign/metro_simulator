use bencher::{benchmark_group, benchmark_main, Bencher};

fn sf_routes_benchmark(bench: &mut Bencher) {
    let (state, mut graph) = sf_routes::setup();
    bench.iter(|| sf_routes::perform_query(&state, &mut graph, &sf_routes::TESTS[0]));
}

benchmark_group!(benches, sf_routes_benchmark);
benchmark_main!(benches);
