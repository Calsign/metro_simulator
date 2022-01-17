load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# ESA GlobCover dataset.
# Download: https://sedac.ciesin.columbia.edu/data/set/gpw-v4-population-density-rev11/data-download

def _workspace_deps():
    http_archive(
        name = "globcover",
        url = "http://due.esrin.esa.int/files/Globcover2009_V2.3_Global_.zip",
        sha256 = "3a5e46b589f6b650759308d4ccb2d62d906a8ffc6f44c6595545e18702a3f7c6",
        build_file_content = """
filegroup(
    name = "data",
    srcs = ["GLOBCOVER_L4_200901_200912_V2.3.tif"],
    visibility = ["//visibility:public"],
)
""",
    )

def _get_deps(latitude, longitude):
    return ["@globcover//:data"]

esa_globcover = struct(
    workspace_deps = _workspace_deps,
    get_deps = _get_deps,
    data = {
        "downsample": 0,
    },
)
