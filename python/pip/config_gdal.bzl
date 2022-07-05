# The versions of the GDAL system package from apt/pacman/etc. must match the version of the GDAL
# package from pip. It is difficult to keep this in sync across different systems, so we make the
# pip version match the system version.

def _config_gdal_impl(repository_ctx):
    result = repository_ctx.execute(["ogrinfo", "--version"])

    if result.return_code != 0:
        print(result.stderr)
        fail("Failed to execute ogrinfo, exit code {}".format(result.return_code))

    # sample output: GDAL 3.4.3, released 2022/04/22
    gdal_version = result.stdout.split(" ", 1)[1].split(",", 1)[0]

    repository_ctx.symlink(
        repository_ctx.attr.requirements,
        "template.txt",
    )

    repository_ctx.template(
        "requirements.txt",
        "template.txt",
        substitutions = {
            "{{GDAL_VERSION}}": gdal_version,
        },
        executable = False,
    )

    repository_ctx.file(
        "BUILD.bazel",
        content = """
exports_files(
    srcs = ["requirements.txt"],
    visibility = ["//visibility:public"],
)
""",
    )

config_gdal = repository_rule(
    implementation = _config_gdal_impl,
    configure = True,
    attrs = {
        "requirements": attr.label(mandatory = True),
    },
)
