load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

# OpenStreetMap.
# Access data: https://www.openstreetmap.org

# We use the Geofabrik extracts.
# Download: http://download.geofabrik.de/

REGIONS = {
    "norcal": struct(
        path = "north-america/us/california/norcal-220101",
        hash = "0d89bd19f58f5ab18c9e44512b351de6e73c56f77d93b63b4471ebac49c018de",
    ),
    "ny": struct(
        path = "north-america/us/new-york-220101",
        hash = None,
    ),
}

def _build_name(region):
    return "osm_{}".format(region)

def _build_url(path):
    return "https://download.geofabrik.de/{}.osm.pbf".format(path)

def _workspace_deps():
    for (region, data) in REGIONS.items():
        http_file(
            name = _build_name(region),
            urls = [_build_url(data.path)],
            sha256 = data.hash,
            downloaded_file_path = "data.osm.pbf",
        )

def _preprocess():
    for (region, data) in REGIONS.items():
        native.genrule(
            name = _build_name(region),
            srcs = ["@{}//file".format(_build_name(region))],
            outs = ["{}.json".format(_build_name(region))],
            cmd = '$(location open_street_map_preprocess) "$<" "$@"',
            exec_tools = [":open_street_map_preprocess"],
            message = "Preprocessing OSM data for {}".format(region),
            visibility = ["//visibility:public"],
        )

def _get_deps(latitude, longitude, regions = None):
    if regions == None:
        fail("Must specify regions with open_street_map.regions")

    return ["//generate/datasets:{}".format(_build_name(region)) for region in regions]

def _regions(regions):
    def get_deps(latitude, longitude):
        return _get_deps(latitude, longitude, regions)

    return struct(
        workspace_deps = open_street_map.workspace_deps,
        get_deps = get_deps,
        data = open_street_map.data,
        regions = open_street_map.regions,
    )

open_street_map = struct(
    workspace_deps = _workspace_deps,
    preprocess = _preprocess,
    get_deps = _get_deps,
    data = {
        "type": "open_street_map",
        "downsample": 3,
    },
    regions = _regions,
)
