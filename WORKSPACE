workspace(name = "metro_simulator")

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# RUST

# `main` branch as of 2022-02-10
RULES_RUST_REF = "c435cf4478fc6e097edc5dba0e71de6608ab77d8"

RUST_VERSION = "1.57.0"

http_archive(
    name = "rules_rust",
    patch_args = ["-p1"],
    patches = [
        "//patches:rules_rust__compile_one_dependency.patch",
        "//patches:rules_rust__android_armeabi-v7a.patch",
    ],
    sha256 = "8e190ea711500bf076f8de6c4c2729ac0d676a992a3d8aefb409f1e786a3f080",
    strip_prefix = "rules_rust-{}".format(RULES_RUST_REF),
    urls = ["https://github.com/bazelbuild/rules_rust/archive/{}.tar.gz".format(RULES_RUST_REF)],
)

load("@rules_rust//rust:repositories.bzl", "rules_rust_dependencies", "rust_register_toolchains", "rust_repository_set")

rules_rust_dependencies()

# register default toolchains
rust_register_toolchains(
    edition = "2021",
    include_rustc_srcs = True,
    version = RUST_VERSION,
)

# Rust support for Android armeabi-v7a
rust_repository_set(
    name = "rust_android_arm",
    edition = "2021",
    exec_triple = "x86_64-unknown-linux-gnu",
    extra_target_triples = ["armv7-linux-androideabi"],
    register_toolchain = True,
    rustfmt_version = RUST_VERSION,
    version = RUST_VERSION,
)

# Rust support for Android aarch64
rust_repository_set(
    name = "rust_android_aarch64",
    edition = "2021",
    exec_triple = "x86_64-unknown-linux-gnu",
    extra_target_triples = ["aarch64-linux-android"],
    register_toolchain = True,
    rustfmt_version = RUST_VERSION,
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
    requirements = "//python/pip:requirements.txt",
)

# MYPY

http_archive(
    name = "mypy_integration",
    sha256 = "9ba22e69e3e8eebb35eb971082cb980becfb2c657d273a26860192d4a7347324",
    strip_prefix = "bazel-mypy-integration-c1193a230e3151b89d2e9ed05b986da34075c280",
    url = "https://github.com/thundergolfer/bazel-mypy-integration/archive/c1193a230e3151b89d2e9ed05b986da34075c280.zip",
)

load(
    "@mypy_integration//repositories:repositories.bzl",
    mypy_repositories = "repositories",
)
load("@mypy_integration//:config.bzl", "mypy_configuration")
load("@mypy_integration//repositories:deps.bzl", mypy_deps = "deps")

mypy_repositories()

mypy_configuration("//python/mypy:mypy_config.ini")

mypy_deps(
    mypy_requirements_file = "//python/mypy:mypy_version.txt",
    python_interpreter = "python3.10",
)

# C++

RULES_CC_VERSION = "0.1.1"

http_archive(
    name = "rules_cc",
    sha256 = None,
    strip_prefix = "rules_cc-{}".format(RULES_CC_VERSION),
    url = "https://github.com/bazelbuild/rules_cc/archive/refs/tags/{}.tar.gz".format(RULES_CC_VERSION),
)

# JAVA

RULES_JAVA_VERSION = "5.0.0"

http_archive(
    name = "rules_java",
    sha256 = "ddc9e11f4836265fea905d2845ac1d04ebad12a255f791ef7fd648d1d2215a5b",
    strip_prefix = "rules_java-{}".format(RULES_JAVA_VERSION),
    url = "https://github.com/bazelbuild/rules_java/archive/refs/tags/{}.tar.gz".format(RULES_JAVA_VERSION),
)

# ANDROID

RULES_ANDROID_VERSION = "0.1.1"

http_archive(
    name = "rules_android",
    sha256 = "6461c1c5744442b394f46645957d6bd3420eb1b421908fe63caa03091b1b3655",
    strip_prefix = "rules_android-{}".format(RULES_ANDROID_VERSION),
    url = "https://github.com/bazelbuild/rules_android/archive/refs/tags/v{}.tar.gz".format(RULES_ANDROID_VERSION),
)

load("@rules_android//android:rules.bzl", "android_ndk_repository", "android_sdk_repository")

# NOTE: requires ANDROID_HOME environment variable to be set.
android_sdk_repository(
    name = "androidsdk",
    api_level = 30,
    build_tools_version = "30.0.3",
)

# NOTE: requires ANDROID_NDK_HOME environment variable to be set.
android_ndk_repository(
    name = "androidndk",
    api_level = 30,
)

register_toolchains("@androidndk//:all")

register_toolchains("//mobile/platform:armv7-linux-androideabi_toolchain")

# DATASETS

load("//generate/datasets:datasets.bzl", "datasets")

datasets.workspace_deps()
