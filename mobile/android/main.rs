use ndk::trace;

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
    app::bootstrap(true);
}
