use std::collections::HashSet;

#[derive(clap::Parser, Debug)]
struct StartEndCoords {
    start_x: u64,
    start_y: u64,
    end_x: u64,
    end_y: u64,
}

#[derive(clap::Parser, Debug)]
enum Operation {
    /// Just construct the base graph.
    Construct,
    /// Construct the base graph and dump it to the output, formatted as dot.
    Dump {
        /// If provided, dump the graph to the file. Otherwise, dump to stdout.
        output: Option<std::path::PathBuf>,
    },
    /// Query a route from the start coords to the end coords.
    Query {
        #[clap(flatten)]
        coords: StartEndCoords,
    },
}

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    operation: Operation,
    load: std::path::PathBuf,
    #[clap(short)]
    metro_lines: Option<Vec<u64>>,
    #[clap(short)]
    highway_segments: Option<Vec<u64>>,
    #[clap(long)]
    has_car: bool,
}

fn dump_graph(graph: &route::InnerGraph, output: &Option<std::path::PathBuf>) {
    match output {
        Some(path) => route::dump_graph(graph, &mut std::fs::File::create(path).unwrap()).unwrap(),
        None => route::dump_graph(graph, &mut std::io::stdout()).unwrap(),
    }
}

fn main() {
    use clap::Parser;
    let args = Args::parse();
    let metro_lines = args.metro_lines.map(HashSet::from_iter);
    let highway_segments = args.highway_segments.map(HashSet::from_iter);
    let car_config = if args.has_car {
        // TODO: support CarConfig::CollectParkedCar
        Some(route::CarConfig::StartWithCar)
    } else {
        None
    };

    let state = engine::state::State::load_file(&args.load).unwrap();

    match args.operation {
        Operation::Construct => {
            let graph = state
                .construct_base_route_graph_filter(metro_lines, highway_segments)
                .unwrap();
        }
        Operation::Dump { output } => {
            let graph = state
                .construct_base_route_graph_filter(metro_lines, highway_segments)
                .unwrap();
            dump_graph(&graph.graph, &output);
        }
        Operation::Query { coords } => {
            let start = state
                .qtree
                .get_address(coords.start_x, coords.start_y)
                .unwrap();
            let end = state.qtree.get_address(coords.end_x, coords.end_y).unwrap();

            let mut graph = state
                .construct_base_route_graph_filter(metro_lines, highway_segments)
                .unwrap();

            let world_state = route::WorldState::new();

            let best = route::best_route(
                &mut graph,
                route::QueryInput {
                    start,
                    end,
                    car_config,
                    start_time: 0,
                },
                &world_state,
            )
            .unwrap();

            match best {
                Some(route) => {
                    route.print();
                }
                None => {
                    println!("No route found.");
                }
            }
        }
    }
}
