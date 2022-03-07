# By default, Bazel will build a different map for each configuration in which a map
# target is instantiated. This includes desktop vs mobile, as well as different
# Android cpu architectures. We don't want to rebuild the maps in these situations,
# so instead we take matters into our own hands

# List of all configuration settings that are changed in the build. #e need to
# transition twice so that we can reset "affected by starlark transition" properly.
# Each value is a (first, second) tuple. The second value is the result of the
# transition. The first value can be any valid value for that configuration setting,
# but it must be different fron the second value in order for "affected by starlark
# transition" to be updated correctly.
ALL_CONFIG_SETTINGS = {
    "//command_line_option:cpu": ("INVALID", "k8"),
    "//command_line_option:compilation_mode": ("opt", "fastbuild"),
    "//command_line_option:fat_apk_cpu": (["INVALID"], []),
    "//command_line_option:crosstool_top": ("INVALID", "@bazel_tools//tools/cpp:toolchain"),
    "//command_line_option:dynamic_mode": ("off", "default"),
    "//command_line_option:Android configuration distinguisher": ("android", "main"),
    "//command_line_option:affected by starlark transition": (["INVALID"], []),
}

# list of all providers that we want to pass through
PROVIDERS = [
    DefaultInfo,
    OutputGroupInfo,
]

def _reset_transition1_impl(settings, attr):
    return {key: value[0] for key, value in ALL_CONFIG_SETTINGS.items()}

def _reset_transition2_impl(settings, attr):
    return {key: value[1] for key, value in ALL_CONFIG_SETTINGS.items()}

_reset_transition1 = transition(
    implementation = _reset_transition1_impl,
    inputs = [],
    outputs = ALL_CONFIG_SETTINGS.keys(),
)

_reset_transition2 = transition(
    implementation = _reset_transition2_impl,
    inputs = [],
    outputs = ALL_CONFIG_SETTINGS.keys(),
)

def _reset_configuration_impl(ctx):
    if len(ctx.attr.actual) != 1:
        fail("shouldn't happen: {}".format(ctx.attr.actual))
    actual = ctx.attr.actual[0]
    output = []
    for provider in PROVIDERS:
        if provider in actual:
            output.append(actual[provider])
    return output

reset_configuration = rule(
    doc = """
    An alias rule that resets all configuration settings to default values.
    Use this to avoid rebuilding an expensive target in different multiple configurations
    when the target doesn't actually depend on the configuration.
    """,
    implementation = _reset_configuration_impl,
    cfg = _reset_transition1,
    attrs = {
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
        "actual": attr.label(
            mandatory = True,
            cfg = _reset_transition2,
        ),
    },
)
