PROVIDERS = [
    DefaultInfo,
    OutputGroupInfo,
    InstrumentedFilesInfo,
    JavaInfo,
]

def _fat_apk_cpu_transition_impl(settings, attr):
    return {
        "//command_line_option:fat_apk_cpu": ",".join(attr.cpus),
    }

_fat_apk_cpu_transition = transition(
    implementation = _fat_apk_cpu_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:fat_apk_cpu"],
)

def _android_cpu_wrapper_impl(ctx):
    actual = ctx.attr.actual
    output = []
    for provider in PROVIDERS:
        if provider in actual:
            output.append(actual[provider])
    return output

android_cpu_wrapper = rule(
    doc = """
    An alias rule that switches to the given fat_apk_cpu.
    """,
    implementation = _android_cpu_wrapper_impl,
    cfg = _fat_apk_cpu_transition,
    attrs = {
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
        "actual": attr.label(mandatory = True),
        "cpus": attr.string_list(mandatory = True),
    },
)
