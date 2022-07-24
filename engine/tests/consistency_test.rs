use std::path::PathBuf;

use uom::si::time::{day, minute};
use uom::si::u64::Time;

use engine::TriggerType;

fn bisect_triggers(
    mut engine: engine::Engine,
    offset: usize,
    partition: usize,
    buckets: usize,
) -> Option<usize> {
    for _ in 0..offset {
        engine
            .single_step()
            .expect("should be no errors in the offset");
    }

    for bucket in 0..buckets {
        for _ in 0..(partition / buckets) {
            if engine.single_step().is_err() {
                return Some(bucket);
            }
        }
        if engine.consistency_check().is_err() {
            return Some(bucket);
        }
    }

    None
}

#[test]
fn consistency_test() {
    let mut engine = engine::Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    engine.init_trigger_queue();

    // It's too slow to run the consistency check every second, so we take bigger steps instead.
    // Then we can reset and take smaller steps once we find an issue.
    let total_time = Time::new::<day>(5).value;
    let step = Time::new::<minute>(15).value;
    assert_eq!(total_time % step, 0);

    let mut snapshot = engine.clone();
    let mut success = true;

    for _ in 0..(total_time / step) {
        engine.time_state.skip_by(step);
        let res = engine.update(0.0, f64::INFINITY);

        #[cfg(feature = "debug")]
        println!("at time {}", engine.time_state.pretty_current_date_time());

        if res.is_err() || engine.consistency_check().is_err() {
            println!(
                "Encountered consistency error at or before {}.",
                engine.time_state.pretty_current_date_time()
            );
            println!("Bisecting since last snapshot to find errant trigger...");
            println!();

            success = false;
            break;
        }

        snapshot = engine.clone();
    }

    if success {
        // ayy we did it
        return;
    }

    drop(engine);

    let mut offset = 0;
    // it's hard to know what size to start with, but this should converge in a reasonable amount of time
    let mut partition = 100000;
    const BUCKETS: usize = 10;

    // expand partition size until we find the consistency error
    loop {
        if bisect_triggers(snapshot.clone(), offset, partition, BUCKETS).is_none() {
            partition *= BUCKETS;
            #[cfg(feature = "debug")]
            println!("expanding partition size to {}", partition);
        } else {
            break;
        }
    }

    while partition > BUCKETS {
        #[cfg(feature = "debug")]
        println!("bisecting - offset: {}, partition: {}", offset, partition);
        let bucket = bisect_triggers(snapshot.clone(), offset, partition, BUCKETS).expect(
            "Reached end of partition window without encountering expected consistency error!",
        );
        offset += (partition / BUCKETS) * bucket;
        partition /= BUCKETS;
    }

    // catch up to where the consistency error is introduced
    #[cfg(feature = "debug")]
    println!("catching up");
    for _ in 0..offset {
        snapshot
            .single_step()
            .expect("should be no errors while catching up");
    }

    assert!(snapshot.consistency_check().is_ok());

    // Restart from the snapshot, stepping through one trigger at a time.
    for _ in 0..partition {
        #[cfg(feature = "debug")]
        println!(
            "single stepping at time {}",
            snapshot.time_state.pretty_current_date_time()
        );

        let trigger = snapshot
            .peek_trigger()
            .expect("expected another trigger")
            .clone();
        // make sure we collect context before stepping
        let context = trigger.debug_context(&snapshot);

        if let Err(err) = snapshot.single_step() {
            println!(
                "Encountered fatal error with no consistency error at {}.",
                snapshot.time_state.pretty_current_date_time()
            );
            println!("Errant trigger: {:#?}", &trigger);
            if let Some(context) = context {
                println!();
                println!("Additional trigger context: {}", context);
            }
            println!();
            println!("Error: {:#?}", err);
            println!();
            panic!("Fatal error");
        }

        if let Err(err) = snapshot.consistency_check() {
            println!(
                "Encountered consistency error at {}.",
                snapshot.time_state.pretty_current_date_time()
            );
            println!();
            println!("Errant trigger: {:#?}", &trigger);
            if let Some(context) = context {
                println!();
                println!("Additional trigger context: {}", context);
            }
            println!();
            println!("Consistency error: {:#?}", err);
            println!();
            panic!("Consistency error detected");
        }
    }

    // This could happen if there is nondeterminism. This should be impossible as long as the RNG in
    // engine (stored as part of the snapshot) is the only source of randomness.
    panic!("Reached end of snapshot window without encountering expected consistency error!");
}
