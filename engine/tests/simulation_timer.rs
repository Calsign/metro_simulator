use std::path::PathBuf;
use std::time::Instant;

use uom::si::time::day;
use uom::si::u64::Time;

fn main() {
    let days = 7;

    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();
    let start_time = Instant::now();
    engine.time_state.skip_by(Time::new::<day>(days).value);
    engine.update(0.0, f64::INFINITY);
    println!(
        "Total time for {} days: {:.4}",
        days,
        start_time.elapsed().as_secs_f64()
    );
}
