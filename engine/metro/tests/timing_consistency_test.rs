use float_cmp::{approx_eq, F64Margin};
use itertools::Itertools;
use std::path::PathBuf;

#[test]
fn timing_consistency_test() {
    // verify that the computed distance-time spline has the same length as the metro line itself

    let margin = F64Margin {
        epsilon: 0.000000001,
        ulps: 0,
    };

    let state = engine::state::State::load_file(&PathBuf::from("maps/sf.json")).unwrap();
    let mut failed = false;
    for metro_line in state.metro_lines.values().sorted() {
        let length = metro_line.get_splines().length * state.config.min_tile_size as f64;
        let dist_spline_length = metro_line
            .get_splines()
            .dist_spline
            .keys()
            .last()
            .unwrap()
            .value;

        println!("Metro line {}: {}", metro_line.id, metro_line.name);
        println!("Length: {:.2}", length);
        println!("Dist spline length: {:.2}", dist_spline_length);
        println!();

        if !approx_eq!(f64, length, dist_spline_length, margin) {
            failed = true;
            println!("INCONSISTENT");
            println!();
        }

        for stop in &metro_line.get_splines().stops {
            let time = *metro_line
                .get_splines()
                .time_map
                .get(&stop.address)
                .unwrap();
            let computed_dist = metro_line
                .get_splines()
                .dist_spline
                .clamped_sample(time)
                .unwrap();
            let expected_dist = *metro_line
                .get_splines()
                .dist_map
                .get(&stop.address)
                .unwrap()
                * state.config.min_tile_size as f64;

            println!("Stop: {:?}", stop);
            println!("Expected distance: {:.2}", expected_dist);
            println!(
                "Computed distance: {:.2} (time: {:.2})",
                computed_dist, time
            );
            if !approx_eq!(f64, expected_dist, computed_dist, margin) {
                println!("NO MATCH!");
                failed = true;
            }
        }

        println!();
    }

    if failed {
        assert!(false, "Some lines are inconsistent");
    }
}
