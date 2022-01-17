load(":esa_globcover.bzl", "esa_globcover")
load(":meta_population_density.bzl", "meta_population_density")

def _workspace_deps():
    esa_globcover.workspace_deps()
    meta_population_density.workspace_deps()

datasets = struct(
    workspace_deps = _workspace_deps,
)
