use std::path::PathBuf;

use uom::si::time::day;
use uom::si::u64::Time;

#[test]
fn dynamics_test() {
    // The purpose of this test is to make sure various dynamics find equilibria at reasonable
    // values after running the simulation for enough time.

    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();

    // give things one week to shake out
    engine.time_state.skip_by(Time::new::<day>(7).value);
    engine.update(0.0, f64::INFINITY).unwrap();

    // test each day for another week
    for day_num in 0..7 {
        engine.time_state.skip_by(Time::new::<day>(1).value);
        engine.update(0.0, f64::INFINITY).unwrap();

        let root = engine.state.qtree.get_root_branch().unwrap();
        let employment_rate = root.fields.population.employment_rate();

        assert!(
            employment_rate > 0.4,
            "employment rate {:.1} < 40% on day {}",
            employment_rate * 100.0,
            day_num,
        );
    }
}
