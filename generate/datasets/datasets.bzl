load(":esa_globcover.bzl", "esa_globcover")
load(":meta_population_density.bzl", "meta_population_density")
load(":census_lodes.bzl", "census_lodes")

def _workspace_deps():
    esa_globcover.workspace_deps()
    meta_population_density.workspace_deps()
    census_lodes.workspace_deps()

datasets = struct(
    workspace_deps = _workspace_deps,
)
