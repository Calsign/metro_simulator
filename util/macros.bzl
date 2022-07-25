load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_shared_library", "rust_test")

RUSTC_FLAGS = [
    "--deny=warnings",
    "--allow=clippy::too-many-arguments",
]

def _patch_kwargs(kwargs, key, lst):
    kwargs[key] = kwargs.get(key, []) + lst

def _patch_rustc_flags(kwargs):
    _patch_kwargs(kwargs, "rustc_flags", RUSTC_FLAGS)

def ms_rust_library(name, **kwargs):
    _patch_rustc_flags(kwargs)
    rust_library(
        name = name,
        **kwargs
    )

def ms_rust_binary(name, **kwargs):
    _patch_rustc_flags(kwargs)
    rust_binary(
        name = name,
        **kwargs
    )

def ms_rust_test(name, **kwargs):
    _patch_rustc_flags(kwargs)
    rust_test(
        name = name,
        **kwargs
    )

def ms_rust_shared_library(name, **kwargs):
    _patch_rustc_flags(kwargs)
    rust_shared_library(
        name = name,
        **kwargs
    )

def ms_rust_benchmark(name, **kwargs):
    _patch_rustc_flags(kwargs)
    _patch_kwargs(kwargs, "tags", ["bench"])
    rust_binary(
        name = name,
        **kwargs
    )
