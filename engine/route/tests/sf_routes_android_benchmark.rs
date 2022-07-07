use ndk::trace;

// NOTE: I haven't written the boilerplate to wrap this in an actual
// android_binary, but this will work if I want to benchmark the route
// planner on Android again.

fn sf_routes_benchmark(bench: &mut bencher::Bencher) {
    use std::io::Read;
    let mut data = String::new();
    ndk_glue::native_activity()
        .asset_manager()
        .open(&std::ffi::CString::new("sf.json").unwrap())
        .expect("json file not found")
        .read_to_string(&mut data)
        .unwrap();
    let state = engine::state::State::load(&data).unwrap();
    let mut graph = state.construct_base_route_graph().unwrap();
    bench.iter(|| sf_routes::perform_query(&state, &mut graph, &sf_routes::TESTS[0]).unwrap());
}

#[cfg_attr(
    target_os = "android",
    ndk_glue::main(
        backtrace = "on",
        logger(level = "debug", tag = "metro_simulator"),
        ndk_glue = "ndk_glue",
    )
)]
fn main() {
    let _trace;
    if trace::is_trace_enabled() {
        _trace = trace::Section::new("metro_simulator main").unwrap();
    }

    benchmark_util::run_benchmark(sf_routes_benchmark).unwrap();
}
