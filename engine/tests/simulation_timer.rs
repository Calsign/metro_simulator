use std::path::PathBuf;
use std::time::Instant;

use uom::si::time::day;
use uom::si::u64::Time;

#[derive(clap::Parser, Debug)]
struct Args {
    days: u64,
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();
    let start_time = Instant::now();
    engine.time_state.skip_by(Time::new::<day>(args.days).value);
    engine.update(0.0, f64::INFINITY);
    println!(
        "Total time for {} days: {:.4}",
        args.days,
        start_time.elapsed().as_secs_f64()
    );
}
