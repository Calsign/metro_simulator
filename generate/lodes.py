import math
import gzip
import csv
import typing as T

import numpy as np
import shapefile
from shapely.geometry import Point, Polygon
from shapely.affinity import affine_transform

from data import Coords, round_to_pow2, centered_box, EQ_KM_PER_DEG


def bbox_contains(outer: T.Tuple[float], inner: T.Tuple[float]):
    (ox1, oy1, ox2, oy2) = outer
    (ix1, iy1, ix2, iy2) = inner

    return (ox1 < ix1 < ox2 or ox1 < ix2 < ox2) and (oy1 < iy1 < oy2 or oy1 < iy2 < oy2)


def read_lodes(dataset: T.Dict[str, T.Any], coords: Coords, max_dim: int):
    shps = set()
    csvs = set()

    for path in dataset["tiles"]:
        if path[-4:] in [".dbf", ".shp", ".shx"]:
            shps.add(path[:-4])
        elif path.endswith(".csv.gz"):
            csvs.add(path)
        else:
            raise Exception("Unrecognized file: {}".format(path))

    assert len(shps) == len(csvs)

    (min_lon, max_lon) = (coords.lon -
                          coords.lon_radius, coords.lon + coords.lon_radius)
    (min_lat, max_lat) = (coords.lat -
                          coords.lat_radius, coords.lat + coords.lat_radius)

    bbox = [min_lon, min_lat, max_lon, max_lat]

    # keep track of all census blocks in the desired area
    census_blocks = {}

    for shp_file in shps:
        sf = shapefile.Reader(shp_file)
        # TODO: once we get pyshp 2.2.0, we can use the bbox filter
        for shapeRec in sf.iterShapeRecords():
            if bbox_contains(bbox, shapeRec.shape.bbox):
                geoid = shapeRec.record["GEOID10"]
                shape = shapeRec.shape.points

                census_blocks[geoid] = shape

    dim = max_dim // (2 ** dataset["data"]["downsample"])
    output = np.zeros([dim, dim])

    # shape transformation matrix; applies translation and scaling to
    # translate from longitude/latitude to x/y coordinates
    xscale = dim / (max_lon - min_lon)
    yscale = dim / (max_lat - min_lat)
    transform = [xscale, 0,
                 0, yscale,
                 -min_lon * xscale, -min_lat * yscale]

    # look at all the LODES data and pull out entries for census blocks
    # we identified earlier
    for csv_file in csvs:
        with gzip.open(csv_file, 'rt', newline='') as f:
            reader = csv.DictReader(f)
            for row in reader:
                geoid = row["w_geocode"]
                if geoid in census_blocks:
                    shape = Polygon(census_blocks[geoid])
                    total = int(row["C000"])

                    transformed = affine_transform(shape, transform)

                    # find grid points within transformed shape
                    (x1, y1, x2, y2) = transformed.bounds
                    in_bounds = []
                    for x in range(max(math.floor(x1), 0), min(math.ceil(x2), dim)):
                        for y in range(max(math.floor(y1), 0), min(math.ceil(y2), dim)):
                            if transformed.contains(Point(x, y)):
                                in_bounds.append((x, y))

                    # distribute total across intersecting points
                    for (x, y) in in_bounds:
                        # TODO: the output is rotated 90 degrees for some reason.
                        # this finagling corrects for that.
                        output[dim - y - 1][x] = total / len(in_bounds)

    return output
