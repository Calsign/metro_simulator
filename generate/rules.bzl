load(":transitions.bzl", "reset_configuration")

EngineConfigProvider = provider(fields = ["data"])

def _engine_config_impl(ctx):
    return EngineConfigProvider(
        data = struct(
            max_depth = ctx.attr.max_depth,
            people_per_sim = ctx.attr.people_per_sim,
            min_tile_size = ctx.attr.min_tile_size,
        ),
    )

engine_config = rule(
    implementation = _engine_config_impl,
    attrs = {
        "max_depth": attr.int(mandatory = True),
        "people_per_sim": attr.int(mandatory = True),
        "min_tile_size": attr.int(mandatory = True),
    },
)

def _generate_map_impl(ctx):
    real_name = ctx.attr.real_name

    output_name = "{}.json".format(real_name)
    output_file = ctx.actions.declare_file(output_name)

    engine_config = ctx.attr.engine_config[EngineConfigProvider].data

    datasets = {}
    dataset_deps = []
    for (dataset_files, key) in ctx.attr.datasets.items():
        if key not in datasets:
            datasets[key] = struct(
                tiles = [],
                # TODO: maybe find a way to do this without the encode/decode
                data = json.decode(ctx.attr.dataset_data[key]),
            )
        for dataset_file in dataset_files.files.to_list():
            datasets[key].tiles.append(dataset_file.path)
            dataset_deps.append(dataset_file)

    map_file = ctx.actions.declare_file("{}.in.json".format(real_name))
    ctx.actions.write(map_file, json.encode(
        struct(
            name = real_name,
            latitude = ctx.attr.latitude,
            longitude = ctx.attr.longitude,
            engine_config = engine_config,
            datasets = datasets,
        ),
    ))

    plot_dir = ctx.actions.declare_directory("{}/plots".format(real_name))
    profile_file = ctx.actions.declare_file("{}/profile".format(real_name))

    save_args = ctx.actions.args()
    save_args.add(map_file)
    save_args.add("--save", output_file)

    ctx.actions.run(
        outputs = [output_file],
        inputs = [map_file] + dataset_deps,
        executable = ctx.executable._generate,
        arguments = [save_args],
        progress_message = "Generating map '{}'".format(real_name),
    )

    plot_args = ctx.actions.args()
    plot_args.add(map_file)
    plot_args.add("--plot-dir", plot_dir.path)
    plot_args.add("--plot", "all")

    ctx.actions.run(
        outputs = [plot_dir],
        inputs = [map_file] + dataset_deps,
        executable = ctx.executable._generate,
        arguments = [plot_args],
        progress_message = "Generating plots for map '{}'".format(real_name),
    )

    profile_args = ctx.actions.args()
    profile_args.add(map_file)
    profile_args.add("--profile-file", profile_file)

    ctx.actions.run(
        outputs = [profile_file],
        inputs = [map_file] + dataset_deps,
        executable = ctx.executable._generate,
        arguments = [profile_args],
        progress_message = "Profiling generation of map '{}'".format(real_name),
    )

    return [
        DefaultInfo(
            files = depset([output_file]),
            runfiles = ctx.runfiles([output_file]),
        ),
        OutputGroupInfo(
            plots = depset([plot_dir]),
            profile = depset([profile_file]),
        ),
    ]

_generate_map = rule(
    implementation = _generate_map_impl,
    attrs = {
        "_generate": attr.label(
            default = "//generate",
            executable = True,
            cfg = "exec",
        ),
        "datasets": attr.label_keyed_string_dict(
            mandatory = True,
            allow_files = True,
        ),
        "dataset_data": attr.string_dict(mandatory = True),
        "latitude": attr.string(mandatory = True),
        "longitude": attr.string(mandatory = True),
        "engine_config": attr.label(
            mandatory = True,
            providers = [EngineConfigProvider],
        ),
        "real_name": attr.string(mandatory = True),
    },
)

def _parse_lat_lon(lat, lon):
    if lat[-1] not in ["N", "S"]:
        fail("Latitude must be N or S: {}".format(lat))
    if lon[-1] not in ["W", "E"]:
        fail("Longitude must be W or E: {}".format(lon))

    latf = float(lat[:-1])
    lonf = float(lon[:-1])

    if lat[-1] == "S":
        latf *= -1
    if lon[-1] == "W":
        lonf *= -1

    if latf < -90 or latf > 90:
        fail("Latitude out of range [90S, 90N]: {}".format(lat))
    if lonf < -180 or lonf > 180:
        fail("Longitude out of range [180W, 180E]: {}".format(lon))

    return (latf, lonf)

def map(name, latitude, longitude, engine_config, datasets, visibility = ["//visibility:public"]):
    (lat, lon) = _parse_lat_lon(latitude, longitude)

    dataset_map = {}
    dataset_data_map = {}
    for (key, dataset) in datasets.items():
        for dep in dataset.get_deps(lat, lon):
            dataset_map[dep] = key
        dataset_data_map[key] = json.encode(dataset.data)

    inner_name = "_{}__inner".format(name)
    _generate_map(
        name = inner_name,
        real_name = name,
        latitude = latitude,
        longitude = longitude,
        datasets = dataset_map,
        dataset_data = dataset_data_map,
        engine_config = engine_config,
        # don't build this automatically since it will be in the wrong configuration
        tags = ["manual"],
    )

    reset_configuration(
        name = name,
        actual = inner_name,
        visibility = visibility,
    )
