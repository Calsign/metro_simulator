def _generate_map_impl(ctx):
    output_name = "{}.json".format(ctx.label.name)
    output_file = ctx.actions.declare_file(output_name)

    map_file = ctx.file.map_file

    args = ctx.actions.args()
    args.add(map_file)
    args.add("--save", output_file)

    ctx.actions.run(
        outputs = [output_file],
        inputs = [map_file] + ctx.files._datasets,
        executable = ctx.executable._generate,
        arguments = [args],
        progress_message = "Generating map '{}'".format(ctx.label.name),
    )

    return [DefaultInfo(files = depset([output_file]))]

generate_map = rule(
    implementation = _generate_map_impl,
    attrs = {
        "_generate": attr.label(
            default = "//generate",
            executable = True,
            cfg = "exec",
        ),
        "_datasets": attr.label(
            default = "//generate/datasets",
        ),
        "map_file": attr.label(
            mandatory = True,
            allow_single_file = True,
        ),
    },
)

def generate_all_maps(name, visibility = ["//visibility:private"]):
    maps = []
    for map_file in native.glob(["*.toml"]):
        map_name = map_file[:-5]
        generate_map(
            name = map_name,
            map_file = map_file,
            visibility = visibility,
        )
        maps.append(map_name)

    native.filegroup(
        name = name,
        srcs = maps,
        visibility = visibility,
    )
