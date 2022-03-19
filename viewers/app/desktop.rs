#[derive(clap::Parser, Debug)]
struct Args {
    load: std::path::PathBuf,
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let app = app::App::load_file(args.load);

    app::bootstrap(app, false);
}
