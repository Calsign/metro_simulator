load("//generate:rules.bzl", "map")
load("//generate/datasets:esa_globcover.bzl", "esa_globcover")
load("//generate/datasets:meta_population_density.bzl", "meta_population_density")
load("//generate/datasets:census_lodes.bzl", "census_lodes")
load("//generate/datasets:open_street_map.bzl", "open_street_map")

maps = {}

maps["nyc"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.states([
            "ny",
            "nj",
            "ct",
            "pa",
        ]),
        "osm": open_street_map.regions(["ny", "nj", "ct", "pa"]),
    },
    engine_config = ":config",
    latitude = "40.7128N",
    longitude = "74.0060W",
)

maps["dc"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.states([
            "dc",
            "va",
            "md",
            "wv",
        ]),
        "osm": open_street_map.regions(["dc", "md", "va"]),
    },
    engine_config = ":config",
    latitude = "38.9072N",
    longitude = "77.0369W",
)

maps["sf"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.states(["ca"]),
        "osm": open_street_map.regions(["norcal"]),
    },
    engine_config = ":config",
    latitude = "37.7749N",
    longitude = "122.4194W",
)

maps["albany"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.states(["ny"]),
        "osm": open_street_map.regions(["ny"]),
    },
    engine_config = ":mini_config",
    latitude = "42.7334N",
    longitude = "73.8479W",
)

ALL_MAPS = {name: Label("//maps:{}".format(name)) for name in maps.keys()}

def create_maps():
    for name, data in maps.items():
        map(
            name = name,
            datasets = data.datasets,
            engine_config = data.engine_config,
            latitude = data.latitude,
            longitude = data.longitude,
        )
