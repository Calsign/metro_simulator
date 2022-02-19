use std::collections::HashSet;

#[derive(clap::Parser, Debug)]
enum Operation {
    Construct,
    Dump { output: Option<std::path::PathBuf> },
}

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    operation: Operation,
    load: std::path::PathBuf,
    #[clap(short)]
    metro_lines: Option<Vec<u64>>,
}

fn main() {
    use clap::Parser;
    let args = Args::parse();
    let metro_lines = args.metro_lines.map(HashSet::from_iter);

    let state = engine::state::State::load_file(&args.load).unwrap();

    match args.operation {
        Operation::Construct => {
            let graph = state.construct_base_route_graph_filter(metro_lines);
        }
        Operation::Dump { output } => {
            let graph = state.construct_base_route_graph_filter(metro_lines);
            match output {
                Some(path) => graph
                    .dump(&mut std::fs::File::create(path).unwrap())
                    .unwrap(),
                None => graph.dump(&mut std::io::stdout()).unwrap(),
            }
        }
    }
}
