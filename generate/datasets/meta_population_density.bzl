load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# Meta/Facebook population density dataset.
# Download: https://data.humdata.org/organization/facebook?q=high%20resolution%20population%20density

MIN_LAT = 38
MIN_LON = -170
TILE_SIZE = 10

# Unfortunately, each download link has some seemingly random garbage in the middle of the url.
# This table maps tile coordinates to garbage and the sha256 hash of the downloaded file.
TILES = {
    # TODO: fill in the rest
    (18, -90): struct(
        garbage = "45c596b5-a8fd-4b7a-97f0-245235f68cbd",
        hash = None,
    ),
    (18, -100): struct(
        garbage = "0ccda306-9f61-47f9-99e6-6b733e09c534",
        hash = None,
    ),
    (28, -80): struct(
        garbage = "114f921c-a821-4067-974f-0482c58b90f7",
        hash = "2abfca2d6b7ac6a9b423de5622d310a95aab971c140eab555ffb9c476859e068",
    ),
    (28, -90): struct(
        garbage = "42fbf8eb-cf6b-4f52-8d9d-5f2edc2aed8a",
        hash = "17135c34a5a3f0f31758fcc4452acf4ef86adc209a76c88ae1009266b2b81766",
    ),
    (28, -120): struct(
        garbage = "58b480a3-afbd-4d1e-bd9f-5ef480bffb61",
        hash = "03c0aa2caabceb5c59aacbede32a3e6d2c79a15eb9cf65451afb79fe0868f052",
    ),
    (28, -130): struct(
        garbage = "619aef38-7ab3-4dff-bcc6-35d4ddef0a26",
        hash = "d81298dc5a975d299ec30bbf16e20c6bdff8d6850b7c06ab352ea263475efd8c",
    ),
    (38, -70): struct(
        garbage = "6f0b12da-ba9c-4a54-8dc9-f6c3beb71547",
        hash = "98c06e56f7f66bb6e76d3ddcc5931d0023f0f4b42915d77ab37a6f95c6cd69bd",
    ),
    (38, -80): struct(
        garbage = "35a6f8c3-4234-4143-9016-103af7c50876",
        hash = "3c534c9759c1d9b9eaa033cafaa9a49ed657003c7bb458d9a649b32df5418b2d",
    ),
    (38, -90): struct(
        garbage = "2f888946-e654-4c17-8885-fa1c09992fc2",
        hash = "475046c768a8a8f3b42b97ec26ff0c4a1b17e0c23a4331c195f2205ee6da62c3",
    ),
    (38, -120): struct(
        garbage = "d68537c2-c0d3-41b1-9c9d-3aabd59ef783",
        hash = "dcfe6be8d366ac7b157062e180a9f2e2179c0877695fa3001bf44102436565b5",
    ),
    (38, -130): struct(
        garbage = "94169896-44c8-436b-bbb0-4b56d5ed9e17",
        hash = "a0f2dd208f98205986a9eaaab43e1ad57366d3804972f57baec51d977d0289ab",
    ),
}

def _check_lat_lon(lat, lon):
    if (lat, lon) not in TILES:
        fail("No tile for ({}, {}).".format(lat, lon))

def _build_filename(lat, lon):
    return "population_usa{}_{}_2019-07-01.tif".format(lat, lon)

def _build_url(lat, lon):
    _check_lat_lon(lat, lon)
    tile = TILES[(lat, lon)]
    return "https://data.humdata.org/dataset/eec3a01f-5237-4896-8059-a6be193ca964/resource/{}/download/{}.zip" \
        .format(tile.garbage, _build_filename(lat, lon))

def _build_name(lat, lon):
    return "meta_population_density_{}_{}".format(lat, lon)

def _workspace_deps():
    for ((lat, lon), tile) in TILES.items():
        http_archive(
            name = _build_name(lat, lon),
            url = _build_url(lat, lon),
            sha256 = tile.hash,
            build_file_content = """
filegroup(
    name = "data",
    srcs = ["{}"],
    visibility = ["//visibility:public"],
)
""".format(_build_filename(lat, lon)),
        )

def _preprocess():
    pass

def floor(x):
    if x > 0:
        return int(x)
    else:
        return int(x) - 1

def ceil(x):
    if x < 0:
        return int(x)
    else:
        return int(x) + 1

def _get_dep(latitude, longitude):
    lat = floor((latitude - MIN_LAT) / float(TILE_SIZE)) * TILE_SIZE + MIN_LAT
    lon = floor((longitude - MIN_LON) / float(TILE_SIZE)) * TILE_SIZE + MIN_LON

    _check_lat_lon(lat, lon)

    return "@{}//:data".format(_build_name(lat, lon))

def _get_deps(latitude, longitude):
    # NOTE: overestimate to be safe.
    # This lon_rad should be good for cities with |latitude| < 60, as cos(60) = 0.5.
    lon_rad = 4
    lat_rad = 2

    # NOTE: use a dict to remove duplicates since starlark doesn't have sets
    deps = {}
    for lat in (latitude - lat_rad, latitude + lat_rad):
        for lon in (longitude - lon_rad, longitude + lon_rad):
            dep = _get_dep(lat, lon)
            deps[dep] = None

    return list(deps.keys())

meta_population_density = struct(
    workspace_deps = _workspace_deps,
    preprocess = _preprocess,
    get_deps = _get_deps,
    data = {
        "type": "geotiff",
        "downsample": 2,
    },
)
