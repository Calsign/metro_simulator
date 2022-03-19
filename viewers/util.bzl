load("//maps:maps.bzl", "ALL_MAPS")

def all_maps(target, name_prefix = "", arg_name = "--"):
    """
    Creates a separate version of a given viewer target for each map.
    For example, creates a target named "sf" that passes the San Francisco map to the viewer.
    """

    for name, map in ALL_MAPS.items():
        native.sh_binary(
            name = name_prefix + name,
            srcs = ["//viewers:map_wrapper.sh"],
            args = [
                "$(location {})".format(target),
                arg_name,
                "{}/{}".format(map.package, map.name),
            ],
            data = [
                target,
                map,
            ],
            deps = ["@bazel_tools//tools/bash/runfiles"],
        )
