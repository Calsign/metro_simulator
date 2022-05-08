load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")

# US Census LODES dataset.
# Download (LODES): https://lehd.ces.census.gov/data/
# Download (shapefiles): https://www.census.gov/geographies/mapping-files/time-series/geo/tiger-line-file.html

# ANSI state codes available here: https://www.census.gov/library/reference/code-lists/ansi/ansi-codes-for-states.html
STATES = {
    "ca": struct(
        ansi_code = 6,
        shapefile_hash = "b8be0fb58cc5c6d600fb578238501ed4e390b7a87189b49ac6f217ff544cb31d",
        lodes_hash = "304691ca7e638c2501c893ada5f15ddde2e96dd8e83d6ba74d5a5e7e5705c18d",
    ),
    "ct": struct(
        ansi_code = 9,
        shapefile_hash = "3fd15012e4931efe8ec9e3926b68020560d752267bbb6517015de64baaecb8e3",
        lodes_hash = "22097eea63881212cbb4a1168381df4728b3240e06db31f82a424202f3b4c0a2",
    ),
    "de": struct(
        ansi_code = 10,
        shapefile_hash = None,
        lodes_hash = None,
    ),
    "dc": struct(
        ansi_code = 11,
        shapefile_hash = "bbd8b283cec8b56c336cd79c1de6b2868f9f3ad48ecd20a098fee9d0a6bcb15b",
        lodes_hash = "7cdf15d16bc20d859078bcde10e1bc513c19a7a8c883e8f0065638d7d5472d2f",
    ),
    "il": struct(
        ansi_code = 17,
        shapefile_hash = "21a2a9f4b83b406a8aa9775dbee9bbf6cd417cef638ff270aacbeb3f2a3cca34",
        lodes_hash = "a0bd9d04dc6f18b65b2902cf98a8cc8f198f63d1f141564a4be8680d0320611a",
    ),
    "in": struct(
        ansi_code = 18,
        shapefile_hash = "42a46c32d85f6980e5ec58ad079260735c540500b24b811253dec904855ea250",
        lodes_hash = "18a2ce41c4a609ba1969dbd31be32c3b7aba25edf674e7037d74f8856822fa53",
    ),
    "me": struct(
        ansi_code = 23,
        shapefile_hash = None,
        lodes_hash = None,
    ),
    "md": struct(
        ansi_code = 24,
        shapefile_hash = "daa4baca6f3cb5b6760498df74dcbae9f14f9e370e877d37c236209d5b410e5b",
        lodes_hash = "c9fc8f14f35d2c8aca62867f27e76e4bde4f951b88bbd28bfc33ed69a7affeff",
    ),
    "mi": struct(
        ansi_code = 26,
        shapefile_hash = "337240a754339f41617791b33e356c7878ff4d900a9811b68cdbad1a0cb76a80",
        lodes_hash = "dfafceafa5435e7d2b310c9d975cd4cf1e3d80236091ac619d4c2ed00d6f8a21",
    ),
    "ma": struct(
        ansi_code = 25,
        shapefile_hash = "62163f87af9646d7e74d37d739568b5903aa5bf4350024f8d18c2bf70d9bf6ab",
        lodes_hash = "ed276e9a90e16bc7bbb851fdb599ab711c33252e0701e2db5e031380209e309d",
    ),
    "nh": struct(
        ansi_code = 33,
        shapefile_hash = "476909ef7095b4f9abb68743ce6ac6971329aa9e40fd4310c566af669f9435a1",
        lodes_hash = "55b45a4dc652664eed34917ad4798cecfaae620ff1bea6e38064c22d5f78cdb8",
    ),
    "nj": struct(
        ansi_code = 34,
        shapefile_hash = "4d59b5e4a5893255ed96ab85982d6cd119f9a9e589ba692c6bc3ab53bb0755cb",
        lodes_hash = "97bbd7476b78af833a5696cbbd0917af2f48b53f2537d59376711f7121e2576e",
    ),
    "ny": struct(
        ansi_code = 36,
        shapefile_hash = "31dd9c236702b7146401038396fc91a6386bb3b6671bdb6cc33a781300a336c9",
        lodes_hash = "2b12924b4a93a1e2947713d687a7bc1afce148e901d3a705dc467540d48c9f94",
    ),
    "pa": struct(
        ansi_code = 42,
        shapefile_hash = "750a18bf1257c8e6d508e67123c8d02e3aa735fda35b3d8905342f79edcad590",
        lodes_hash = "959b76aaeda61e142d8ad579f3efbe19973316ab071ceab90978455509da420a",
    ),
    "ri": struct(
        ansi_code = 44,
        shapefile_hash = "0cf793febe8a610622aad8b8038948b0c0c2fb916c6149bc238febd5d4902ac7",
        lodes_hash = "41534e17b1a4d4814c02b4c453c2a078623b60c45e0cd16658919e929c353892",
    ),
    "va": struct(
        ansi_code = 51,
        shapefile_hash = "eff2b9549f8718a2e09b22055ebb5059db1e087c8cd22be0204ad45f3e3ecca4",
        lodes_hash = "36cba899ad991029ea4f24ce7c2ab69d4baaf38f8a9969c5eb2eb4583a2b70ec",
    ),
    "wv": struct(
        ansi_code = 54,
        shapefile_hash = "53641d3d31de90a58dea7b95d9fec3a324ab605eb87fda46c04d3c4441ffa883",
        lodes_hash = "6164771269e1fe1218a80db39520f579140a28619eb80687f58bf19788218e90",
    ),
    "wi": struct(
        ansi_code = 55,
        shapefile_hash = "7f235bf3ace2dd998fa6c17963ae48017375a0b991933754e672a4101dccca93",
        lodes_hash = "9aea10879d29010a011c2134c3ccc672d4a373f076bfc6f68f8be8db26e2a76e",
    ),
}

def _build_shapefile_name(state):
    return "census_tiger_{}".format(state)

def zfill(n, digits):
    s = str(n)
    return "".join(["0"] * (digits - len(s))) + s

def _build_shapefile_filename(ansi_code):
    return "tl_2019_{}_tabblock10".format(zfill(ansi_code, 2))

def _build_shapefile_url(ansi_code):
    return "https://www2.census.gov/geo/tiger/TIGER2019/TABBLOCK/{}.zip" \
        .format(_build_shapefile_filename(ansi_code))

def _build_lodes_name(state):
    return "census_lodes_{}".format(state)

def _build_lodes_url(state):
    return "https://lehd.ces.census.gov/data/lodes/LODES7/{0}/wac/{0}_wac_S000_JT00_2019.csv.gz" \
        .format(state)

def _workspace_deps():
    for (state, data) in STATES.items():
        http_archive(
            name = _build_shapefile_name(state),
            url = _build_shapefile_url(data.ansi_code),
            sha256 = data.shapefile_hash,
            build_file_content = """
filegroup(
    name = "data",
    srcs = [
        "{0}.dbf",
        "{0}.shp",
        "{0}.shx",
    ],
    visibility = ["//visibility:public"],
)
""".format(_build_shapefile_filename(data.ansi_code)),
        )

        http_file(
            name = _build_lodes_name(state),
            urls = [_build_lodes_url(state)],
            sha256 = data.lodes_hash,
            downloaded_file_path = "data.csv.gz",
        )

def _preprocess():
    pass

def _get_deps(latitude, longitude, states = None):
    if states == None:
        fail("Must specify states using census_lodes.states")

    deps = []

    for state in states:
        data = STATES[state]
        deps.append("@{}//:data".format(_build_shapefile_name(state)))
        deps.append("@{}//file".format(_build_lodes_name(state)))

    return deps

def _states(states):
    def get_deps(latitude, longitude):
        return _get_deps(latitude, longitude, states)

    return struct(
        workspace_deps = census_lodes.workspace_deps,
        get_deps = get_deps,
        data = census_lodes.data,
        states = census_lodes.states,
    )

census_lodes = struct(
    workspace_deps = _workspace_deps,
    preprocess = _preprocess,
    get_deps = _get_deps,
    data = {
        "type": "lodes",
        "downsample": 3,
    },
    states = _states,
)
