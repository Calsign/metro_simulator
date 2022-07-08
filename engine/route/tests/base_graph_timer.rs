use std::path::PathBuf;
use std::time::Instant;

use engine::{BaseGraph, Engine};

fn main() {
    let engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    let start_time = Instant::now();
    BaseGraph::construct_base_graph(&engine.state).unwrap();
    println!(
        "Total time to construct base graph: {:.4}",
        start_time.elapsed().as_secs_f64(),
    );
}
