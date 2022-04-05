load("@rules_rust//crate_universe:defs.bzl", "crate")

CRATES = {
    # utility
    "anyhow": "1.0",
    "thiserror": "1.0",
    "lazy_static": "1.4",
    "once_cell": "1.10.0",
    "derive_more": "0.99",
    "enum_dispatch": "0.3",
    "chrono": "0.4",
    "itertools": "0.10",
    "float-cmp": dict(
        git = "https://github.com/mikedilger/float-cmp",
        rev = "418c5d9d339268f355363ea7cf6c546e69d63b7b",
    ),
    "bencher": "0.1.5",

    # serde
    "serde": dict(
        version = "1.0",
        features = ["derive"],
    ),
    "serde_json": "1.0",
    "toml": "0.5",

    # math
    "cgmath": dict(
        version = "0.18",
        features = ["serde"],
    ),
    "petgraph": "0.6",
    "splines": dict(
        version = "4.0",
        features = ["cgmath", "serde"],
    ),

    # cli
    "clap": dict(
        version = "3",
        features = ["derive"],
    ),

    # plotting
    "plotters": "0.3.1",
    "plotters-bitmap": "0.3.1",

    # python FFI
    "pyo3": dict(
        version = "0.15",
        features = ["extension-module"],
    ),

    # java FFI
    "jni": "0.19.0",

    # druid graphics
    # NOTE: rules_rust 0.1.0 chokes trying to generate build files for druid.
    # "druid": dict(
    #     git = "https://github.com/linebender/druid",
    #     # master as of 2022-01-03
    #     rev = "3790463cf4e724719dc0c1867afe59c3f2d22b3b",
    #     features = ["im"],
    # ),

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
