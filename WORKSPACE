workspace(name = "metro_simulator")

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

GAZELLE_RUST_COMMIT = "ca7ef4f31bc98f5d33ae969987d4163e5c5054a3"

http_archive(
    name = "gazelle_rust",
    sha256 = "db1e92f878479452c3dbd2c3f5327625719f574b18c635468b10592d4c78b6b1",
    strip_prefix = "gazelle_rust-{}".format(GAZELLE_RUST_COMMIT),
    url = "https://github.com/Calsign/gazelle_rust/archive/{}.zip".format(GAZELLE_RUST_COMMIT),
)

# RUST

# 0.15.0, 2022-12-28
RULES_RUST_VERSION = "0.15.0"

RUST_VERSION = "1.66.0"

http_archive(
    name = "rules_rust",
    patch_args = ["-p1"],
    patches = [
        # needed by gazelle_rust
        "@gazelle_rust//patches:rules_rust_p1.patch",
        # https://github.com/bazelbuild/rules_rust/pull/1733
        "//patches:rules_rust__crate_universe_disable_pipelining.patch",
    ],
    sha256 = "5c2b6745236f8ce547f82eeacbbcc81d736734cc8bd92e60d3e3cdfa6e167bb5",
    urls = [
        "https://mirror.bazel.build/github.com/bazelbuild/rules_rust/releases/download/{0}/rules_rust-v{0}.tar.gz".format(RULES_RUST_VERSION),
        "https://github.com/bazelbuild/rules_rust/releases/download/{0}/rules_rust-v{0}.tar.gz".format(RULES_RUST_VERSION),
    ],
)

load("@rules_rust//rust:repositories.bzl", "rules_rust_dependencies", "rust_register_toolchains", "rust_repository_set")

rules_rust_dependencies()

# register default toolchains
rust_register_toolchains(
    edition = "2021",
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

# CRATE UNIVERSE

load("@rules_rust//crate_universe:repositories.bzl", "crate_universe_dependencies")

# NOTE: need to bootstrap in order to patch cargo_bazel
crate_universe_dependencies(bootstrap = True)

load("@rules_rust//crate_universe:defs.bzl", "crate", "crates_repository", "splicing_config")
load("//cargo:crates.bzl", "ANNOTATIONS", "all_crates")

crates_repository(
    name = "crates",
    annotations = ANNOTATIONS,
    cargo_lockfile = "//cargo:cargo.lock",
    generator = "@cargo_bazel_bootstrap//:cargo-bazel",
    lockfile = "//cargo:crate_universe.lock",
    packages = all_crates(),
    quiet = False,
    splicing_config = splicing_config(resolver_version = "2"),
    supported_platform_triples = [
        "i686-apple-darwin",
        "i686-pc-windows-msvc",
        "i686-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "aarch64-apple-ios",
        "aarch64-linux-android",
        "aarch64-unknown-linux-gnu",
        "arm-unknown-linux-gnueabi",
        "armv7-unknown-linux-gnueabi",
        "i686-linux-android",
        "i686-unknown-freebsd",
        "powerpc-unknown-linux-gnu",
        "s390x-unknown-linux-gnu",
        "wasm32-unknown-unknown",
        "wasm32-wasi",
        "x86_64-apple-ios",
        "x86_64-linux-android",
        "x86_64-unknown-freebsd",
    ],
)

load("@crates//:defs.bzl", "crate_repositories")

crate_repositories()

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

register_toolchains("//python:python_toolchain")

# GDAL

load("//python/pip:config_gdal.bzl", "config_gdal")

config_gdal(
    name = "local_config_gdal",
    requirements = "//python/pip:requirements.txt",
)

# PIP

load("@rules_python//python:pip.bzl", "pip_install")

pip_install(
    name = "pip_pkgs",
    python_interpreter = "python3.10",
    requirements = "@local_config_gdal//:requirements.txt",
)

# MYPY

http_archive(
    name = "mypy_integration",
    patch_args = ["-p1"],
    patches = [
        # https://github.com/bazel-contrib/bazel-mypy-integration/pull/72
        "//patches:bazel_mypy_integration__support_newer_version.patch",
    ],
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

# SKYLIB

http_archive(
    name = "bazel_skylib",
    sha256 = "f7be3474d42aae265405a592bb7da8e171919d74c16f082a5457840f06054728",
    url = "https://github.com/bazelbuild/bazel-skylib/releases/download/1.2.1/bazel-skylib-1.2.1.tar.gz",
)

# PACKAGING

http_archive(
    name = "rules_pkg",
    sha256 = "8a298e832762eda1830597d64fe7db58178aa84cd5926d76d5b744d6558941c2",
    urls = [
        "https://mirror.bazel.build/github.com/bazelbuild/rules_pkg/releases/download/0.7.0/rules_pkg-0.7.0.tar.gz",
        "https://github.com/bazelbuild/rules_pkg/releases/download/0.7.0/rules_pkg-0.7.0.tar.gz",
    ],
)

load("@rules_pkg//:deps.bzl", "rules_pkg_dependencies")

rules_pkg_dependencies()

# DATASETS

load("//generate/datasets:datasets.bzl", "datasets")

datasets.workspace_deps()

# GAZELLE

load("@gazelle_rust//:deps1.bzl", "gazelle_rust_dependencies1")

gazelle_rust_dependencies1()

load("@gazelle_rust//:deps2.bzl", "gazelle_rust_dependencies2")

gazelle_rust_dependencies2()
