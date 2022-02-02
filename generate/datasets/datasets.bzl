load(":esa_globcover.bzl", "esa_globcover")
load(":meta_population_density.bzl", "meta_population_density")
load(":census_lodes.bzl", "census_lodes")
load(":open_street_map.bzl", "open_street_map")

ALL_DATASETS = [
    esa_globcover,
    meta_population_density,
    census_lodes,
    open_street_map,
]

def _workspace_deps():
    for dataset in ALL_DATASETS:
        dataset.workspace_deps()

def _preprocess():
    for dataset in ALL_DATASETS:
        dataset.preprocess()

datasets = struct(
    workspace_deps = _workspace_deps,
    preprocess = _preprocess,
)
