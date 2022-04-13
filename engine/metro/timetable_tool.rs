#[derive(clap::Parser, Debug)]
enum Operation {
    TimeTable,
    SpeedPlot { output: std::path::PathBuf },
    SpeedBounds { output: std::path::PathBuf },
    ComputeDistSpline,
}

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    operation: Operation,
    load: std::path::PathBuf,
    metro_line: u64,
}

fn plot(
    data: &Vec<(f64, f64)>,
    output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use plotters::prelude::*;

    let root_area = BitMapBackend::new(output, (1600, 400)).into_drawing_area();
    root_area.fill(&WHITE)?;

    let min_x = data.iter().map(|(x, _)| *x).reduce(f64::min).unwrap();
    let max_x = data.iter().map(|(x, _)| *x).reduce(f64::max).unwrap();
    let min_y = data.iter().map(|(_, y)| *y).reduce(f64::min).unwrap();
    let max_y = data.iter().map(|(_, y)| *y).reduce(f64::max).unwrap();

    let mut chart = ChartBuilder::on(&root_area)
        .margin(5)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)?;

    chart.configure_mesh().draw()?;

    chart.draw_series(LineSeries::new(data.iter().map(|(x, y)| (*x, *y)), &BLACK))?;

    root_area.present()?;
    Ok(())
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let state = engine::state::State::load_file(&args.load).unwrap();
    let metro_line = state.metro_lines.get(&args.metro_line).unwrap();

    println!("Loaded metro line: {}", metro_line.name);

    let speed_keys =
        metro::timing::speed_keys(metro_line.get_keys(), state.config.min_tile_size as f64);

    match args.operation {
        Operation::TimeTable => {
            let timetable = metro::timing::timetable(&speed_keys);
            for (station, time) in timetable {
                println!("{}: {}", time.round() as u64, station.name);
            }
        }
        Operation::SpeedPlot { output } => {
            plot(
                &speed_keys.iter().map(|key| (key.t, key.v)).collect(),
                &output,
            )
            .unwrap();
        }
        Operation::SpeedBounds { output } => {
            let speed_bounds = metro::timing::speed_bounds(
                metro_line.get_keys(),
                state.config.min_tile_size as f64,
            );
            plot(
                &speed_bounds
                    .iter()
                    .map(|bound| (bound.t, bound.b))
                    .collect(),
                &output,
            )
            .unwrap();
        }
        Operation::ComputeDistSpline => {
            println!(
                "metro line length: {}",
                metro_line.get_splines().length * state.config.min_tile_size as f64
            );
            let dist_spline = metro::timing::dist_spline(&speed_keys);
        }
    };
}
