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
        "employment": census_lodes.construct([
            "ny",
            "nj",
            "ct",
            "pa",
        ]),
        "osm": open_street_map.construct(
            regions = ["ny", "nj", "ct", "pa"],
            subway_speeds = {
                "NYC Subway": "55 mph",
                "NJ Transit": "80 mph",
                "Metro-North Railroad": "70 mph",
                "LIRR": "80 mph",
                "HBLR": "55 mph",
                "Staten Island Railway": "70 mph",
                "PATH": "55 mph",
                "CTrail": "90 mph",
                "AirTrain JFK": "60 mph",
                "SEPTA": "55 mph",
            },
        ),
    },
    engine_config = ":config",
    latitude = "40.7128N",
    longitude = "74.0060W",
)

maps["dc"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.construct([
            "dc",
            "va",
            "md",
            "wv",
        ]),
        "osm": open_street_map.construct(
            regions = ["dc", "md", "va"],
            subway_speeds = {
                "Washington Metro": "59 mph",
                "MARC": "125 mph",
                "Metro SubwayLink": "70 mph",
                "Light RailLink": "60 mph",
                "Virginia Railway Express": "60 mph",
            },
        ),
    },
    engine_config = ":config",
    latitude = "38.9072N",
    longitude = "77.0369W",
)

maps["sf"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.construct(
            states = ["ca"],
        ),
        "osm": open_street_map.construct(
            regions = ["norcal"],
            subway_speeds = {
                "Muni": "50 mph",
                "Caltrain": "79 mph",
                "BART": "70 mph",
                "VTA": "55 mph",
                "SMART": "79 mph",
            },
        ),
    },
    engine_config = ":config",
    latitude = "37.7749N",
    longitude = "122.4194W",
    cleaner = "cleaners/sf.py",
)

maps["boston"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.construct(
            states = ["ma", "nh", "ri"],
        ),
        "osm": open_street_map.construct(
            regions = ["ma", "nh", "ri"],
            subway_speeds = {
                "MBTA": "60 mph",
            },
        ),
    },
    engine_config = ":config",
    latitude = "42.3601N",
    longitude = "71.0589W",
)

maps["chicago"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.construct(
            states = ["il", "wi", "in", "mi"],
        ),
        "osm": open_street_map.construct(
            regions = ["il", "wi", "in", "mi"],
            subway_speeds = {
                "CTA": "55 mph",
                "Metra": "79 mph",
                "NICTD": "79 mph",
                "ATS": "50 mph",
            },
        ),
    },
    engine_config = ":config",
    latitude = "41.8781N",
    longitude = "87.6298W",
)

maps["albany"] = struct(
    datasets = {
        "terrain": esa_globcover,
        "population": meta_population_density,
        "employment": census_lodes.construct(
            states = ["ny"],
        ),
        "osm": open_street_map.construct(
            regions = ["ny"],
        ),
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
            cleaner = getattr(data, "cleaner", None),
            cleaner_deps = getattr(data, "cleaner_deps", []),
            tags = ["manual"],
        )
