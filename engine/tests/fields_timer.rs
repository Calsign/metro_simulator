use std::path::PathBuf;
use std::time::Instant;

use engine::Engine;

fn main() {
    let mut engine = Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();

    let start_time = Instant::now();
    engine.update_fields().unwrap();

    println!(
        "Total time for fields computation: {:.4}",
        start_time.elapsed().as_secs_f64()
    );
}
