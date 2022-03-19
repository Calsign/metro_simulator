use ndk::trace;

fn load_map(name: &str) -> String {
    use std::io::Read;
    let mut data = String::new();
    ndk_glue::native_activity()
        .asset_manager()
        .open(&std::ffi::CString::new(name).unwrap())
        .expect("json file not found")
        .read_to_string(&mut data)
        .unwrap();
    return data;
}

#[cfg_attr(
    target_os = "android",
    ndk_glue::main(
        backtrace = "on",
        logger(level = "debug", tag = "metro_simulator"),
        ndk_glue = "ndk_glue",
    )
)]
fn main() {
    let _trace;
    if trace::is_trace_enabled() {
        _trace = trace::Section::new("metro_simulator main").unwrap();
    }

    // TODO: don't hard-code map
    // eventually we will want a menu and the ability to select a map
    let app = app::App::load_str(&load_map("sf.json"));

    app::bootstrap(app, true);
}
