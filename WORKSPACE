load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# RUST

# `main` branch as of 2021-12-21
RULES_RUST_REF = "6630fd5b6b7fe143ea09f80f64dc20e3514495b4"

RUST_VERSION = "1.57.0"

http_archive(
    name = "rules_rust",
    sha256 = "285a4d967abf3739f1dcb34e2f5a7d056dde1de3e3bb3f0145522e9b9433cba9",
    strip_prefix = "rules_rust-{}".format(RULES_RUST_REF),
    urls = ["https://github.com/bazelbuild/rules_rust/archive/{}.tar.gz".format(RULES_RUST_REF)],
)

load("@rules_rust//rust:repositories.bzl", "rust_repositories")

rust_repositories(
    edition = "2021",
    include_rustc_srcs = True,
    version = RUST_VERSION,
)

# CARGO RAZE

CARGO_RAZE_VERSION = "0.14.0"

http_archive(
    name = "cargo_raze",
    sha256 = "92a4116f82938027a19748580d2ec8d2d06801c868503b1b195bd312ad608d19",
    strip_prefix = "cargo-raze-{}".format(CARGO_RAZE_VERSION),
    url = "https://github.com/google/cargo-raze/archive/v{}.tar.gz".format(CARGO_RAZE_VERSION),
)

load("@cargo_raze//:repositories.bzl", "cargo_raze_repositories")

cargo_raze_repositories()

load("@cargo_raze//:transitive_deps.bzl", "cargo_raze_transitive_deps")

cargo_raze_transitive_deps()

# CARGO DEPENDENCIES VIA RAZE

load("//cargo/pkgs:crates.bzl", "raze_fetch_remote_crates")

raze_fetch_remote_crates()

# RUST ANALYZER (rust-project.json)

load("@rules_rust//tools/rust_analyzer:deps.bzl", "rust_analyzer_deps")

rust_analyzer_deps()

# PYTHON

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

http_archive(
    name = "rules_python",
    sha256 = "cd6730ed53a002c56ce4e2f396ba3b3be262fd7cb68339f0377a45e8227fe332",
    url = "https://github.com/bazelbuild/rules_python/releases/download/0.5.0/rules_python-0.5.0.tar.gz",
)

# PIP

load("@rules_python//python:pip.bzl", "pip_install")

pip_install(
    name = "pip_pkgs",
    requirements = "//pip:requirements.txt",
)
