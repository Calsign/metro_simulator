load("@rules_rust//crate_universe:defs.bzl", "crate")

CRATES = {
    # utility
    "anyhow": "1.0",
    "thiserror": "1.0",
    "lazy_static": "1.4",
    "once_cell": "1.10.0",
    "derive_more": "0.99",
    "derivative": "2.2.0",
    "enum-iterator": "0.8.1",
    "enum_dispatch": "0.3",
    "enum-kinds": "0.5.1",
    "chrono": dict(
        version = "0.4",
        features = ["serde"],
    ),
    "itertools": "0.10",
    "float-cmp": dict(
        git = "https://github.com/mikedilger/float-cmp",
        rev = "418c5d9d339268f355363ea7cf6c546e69d63b7b",
    ),
    "ordered-float": "3.0.0",
    "bencher": "0.1.5",

    # serde
    "serde": dict(
        version = "1.0",
        features = ["derive"],
    ),
    "serde_with": "1.14.0",
    "serde_json": "1.0",
    "toml": "0.5",

    # math
    "cgmath": dict(
        version = "0.18",
        features = ["serde"],
    ),
    "petgraph": "0.6",
    "fast_paths": dict(
        git = "https://github.com/easbar/fast_paths",
        # master as of 2022-04-20; we need PR #37
        rev = "6d236d1be5f341071c65ee36c540279986cf7231",
    ),
    "splines": dict(
        version = "4.0",
        features = ["cgmath", "serde"],
    ),
    "line_drawing": "1.0.0",
    "spade": "2.0.0",
    "fastblur": "0.1.1",
    "image": "0.24.2",
    "imageproc": "0.23.0",
    "rand": "0.8.5",
    "rand_chacha": dict(
        version = "0.3.1",
        features = ["serde1"],
    ),
    "rand_distr": "0.4.3",
    "uom": dict(
        version = "0.32.0",
        features = ["u64", "serde"],
    ),
    "num": "0.4.0",

    # parallelism
    "rayon": "1.5.2",
    "threadpool": "1.8.1",
    "crossbeam": "0.8.1",
    "thread_local": "1.1.4",

    # generating flamegraphs (not used in the code, just the executable is used)
    "flamegraph": "0.6.1",

    # cli
    "clap": dict(
        version = "3",
        features = ["derive"],
    ),

    # plotting
    "plotters": "0.3.1",
    "plotters-bitmap": "0.3.1",
    "tabled": "0.8.0",

    # python FFI
    "pyo3": dict(
        version = "0.15",
        features = ["extension-module"],
    ),

    # java FFI
    "jni": "0.19.0",

    # druid graphics
    # NOTE: rules_rust 0.1.0 chokes trying to generate build files for druid.
    "druid": dict(
        git = "https://github.com/linebender/druid",
        # master as of 2022-01-03
        rev = "3790463cf4e724719dc0c1867afe59c3f2d22b3b",
        features = ["im"],
    ),

    # wgpu graphics
    "wgpu": "0.12.0",
    "winit": "0.26.1",
    "env_logger": "0.9.0",
    "log": "0.4.14",
    "pollster": "0.2.5",

    # egui
    "egui": "0.17.0",
    "egui_wgpu_backend": "0.17.0",
    "egui_winit_platform": "0.14.0",

    # android
    "ndk": dict(
        version = "0.6.0",
        features = ["trace"],
    ),
    "ndk-glue": dict(
        # NOTE: must be chosen to be compatible with winit
        version = "0.5.0",
        features = ["logger"],
    ),
    "ndk-context": "0.1.0",
}

def all_crates():
    # simple helper which allows us to write just the version in most cases
    return {
        name: crate.spec(version = data) if type(data) == "string" else crate.spec(**data)
        for name, data in CRATES.items()
    }
