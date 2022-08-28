use std::path::PathBuf;

use uom::si::time::hour;
use uom::si::u64::Time;

#[test]
fn network_mutation_test() {
    // Apply random edits to the networks (highways & railways) and check for consistency errors.

    use rand::seq::IteratorRandom;
    use rand::SeedableRng;

    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();

    // use a RNG to give us some data, but make it deterministic by using a constant seed
    let mut rng = rand_chacha::ChaCha12Rng::seed_from_u64(0);

    const DAYS: usize = 5;
    const HOURS: usize = 6;
    const EDITS: usize = 100;

    for i in 0..(DAYS * HOURS) {
        println!("Beginning iteration {}", i);

        engine
            .time_state
            .skip_by(Time::new::<hour>(HOURS as u64).value);
        engine.update(0.0, f64::INFINITY).unwrap();

        println!("Stepped forward; performing edits");

        // edit some segments
        for _ in 0..EDITS {
            let id = engine
                .state
                .highways
                .segments()
                .values()
                .filter(|segment| segment.change_state.is_staged_active())
                .choose(&mut rng)
                .unwrap()
                .id;
            let _segment = engine.state.highways.edit_segment(id);
        }

        // edit some junctions
        for _ in 0..EDITS {
            let id = engine
                .state
                .highways
                .junctions()
                .values()
                .filter(|junction| junction.change_state.is_staged_active())
                .choose(&mut rng)
                .unwrap()
                .id;
            let _junction = engine.state.highways.edit_junction(id);
        }

        println!("Validating pre-apply");

        engine.state.highways.validate();
        engine.state.railways.validate();

        // TODO: also test railways (not yet possible since we don't handle metro lines correctly)

        engine.apply_change_set();

        println!("Validating post-apply");

        engine.state.highways.validate();
        engine.state.railways.validate();
    }
}
