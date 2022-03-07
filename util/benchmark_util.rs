use bencher::{run_tests_console, Bencher, TestDesc, TestDescAndFn, TestFn, TestOpts};
use std::borrow::Cow;
use std::io::Result;

/// Useful for running a benchmark embedded in another program
pub fn run_benchmark(f: fn(_: &mut Bencher)) -> Result<bool> {
    let bench = TestDescAndFn {
        desc: TestDesc {
            name: Cow::from("benchmark"),
            ignore: false,
        },
        testfn: TestFn::StaticBenchFn(f),
    };
    let benches = vec![bench];
    let test_ops = TestOpts::default();

    run_tests_console(&test_ops, benches)
}
