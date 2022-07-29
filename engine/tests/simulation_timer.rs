use std::path::PathBuf;
use std::time::Instant;

use uom::si::time::{day, hour};
use uom::si::u64::Time;

#[derive(clap::Parser, Debug)]
struct Args {
    days: Option<u64>,
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let days = args.days.unwrap_or(7);

    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();
    let start_time = Instant::now();

    let total = Time::new::<day>(days).value;
    let step_size = Time::new::<hour>(1).value;

    assert_eq!(total % step_size, 0);

    let steps = total / step_size;

    // TODO: Printing will mess up the progress bar. indicatif provides a function for printing
    // above the bar in a pretty way, but then we have to use that everywhere.
    let progress = indicatif::ProgressBar::new(steps);

    for _ in 0..steps {
        engine.time_state.skip_by(step_size);
        engine.update(0.0, f64::INFINITY).unwrap();

        progress.inc(1);
    }

    println!(
        "Total time for {} days: {:.4}",
        days,
        start_time.elapsed().as_secs_f64()
    );

    if engine.trigger_stats.profiling_enabled {
        engine.trigger_stats.print();
        println!();
    }
}
