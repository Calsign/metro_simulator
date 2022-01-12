#!/usr/bin/env python3

import os
import sys
import math
from dataclasses import dataclass
import json
import functools

import numpy as np
from osgeo import gdal
import toml
import argh

import engine

from quadtree import Quadtree


@functools.lru_cache
def runfiles():
    from rules_python.python.runfiles import runfiles
    return runfiles.Create()


@dataclass
class MapConfig:
    engine_config: str
    latitude: str
    longitude: str
    radius: str


def deg_to_sec(deg):
    return math.floor(deg * 120)


def lon_lat_to_secs(lon, lat):
    assert 0 <= lon < 360
    assert 0 <= lat <= 180

    return deg_to_sec(lon), deg_to_sec(lat)


def centered_box(lon, lat, radius):
    (lon, lat) = lon_lat_to_secs(lon, lat)
    radius = deg_to_sec(radius)
    return ((lon - radius, lat - radius), (lon + radius, lat + radius))


def parse_lat_lon(lat, lon):
    assert lat[-1] in ["N", "S"]
    assert lon[-1] in ["W", "E"]

    latf = float(lat[:-1])
    lonf = float(lon[:-1])

    if lat[-1] == "N":
        latf = 90 - latf
    elif lat[-1] == "S":
        latf = 90 + latf

    if lon[-1] == "W":
        lonf = -lonf
    elif lon[-1] == "E":
        lonf = lonf

    return (latf, lonf)


def make_tile(type_, **fields):
    return {
        "tile": {
            "type": type_,
            **fields,
        }
    }


def tile_water(water_grid):
    assert water_grid.shape[0] == water_grid.shape[1]
    dim = water_grid.shape[0]
    assert math.log(dim, 2) % 1 == 0
    depth = int(math.log(dim, 2))

    qtree = Quadtree(max_depth=depth)
    qtree.fill(None)

    # populate with water data from the input array
    def initial(node, data):
        if data.depth == depth:
            node.data = water_grid[data.x][data.y] != 0
    qtree.convolve(initial)

    # collapse groups of water and non-water nodes
    def collapse(node, data):
        if len(node.children) > 0:
            first = node.children[0].data
            if first is not None and all([c.data == first for c in node.children]):
                node.data = first
                node.children = []
    qtree.convolve(collapse, post=True)

    # convert to tiles
    def convert(node, data):
        if len(node.children) == 0:
            if node.data:
                node.data = make_tile("WaterTile")
            else:
                node.data = make_tile("EmptyTile")
    qtree.convolve(convert)

    return qtree


def write_qtree(state, qtree):
    def write(node, data):
        address = engine.Address(data.address)
        if len(node.children) > 0:
            assert len(node.children) == 4
            state.split(address, engine.BranchState(),
                        engine.LeafState(), engine.LeafState(),
                        engine.LeafState(), engine.LeafState())
        else:
            dumped = json.dumps(node.data)
            try:
                state.set_leaf_json(address, dumped)
            except Exception as e:
                print("Dumped json: {}".format(dumped))
                raise e
    qtree.convolve(write)


def main(map_path, population_path=None, plot=False, save=None):
    if plot:
        import matplotlib
        import matplotlib.pyplot as plt

    if population_path is None:
        population_path = runfiles().Rlocation(
            "metro_simulator/generate/datasets/population.tif")

    map_config = MapConfig(**toml.load(map_path))

    state = engine.State(engine.Config(map_config.engine_config))

    gdal.UseExceptions()
    data = gdal.Open(population_path, gdal.GA_ReadOnly)
    band = data.GetRasterBand(1)

    (min_lon, min_lat, _, _, _, _) = data.GetGeoTransform()

    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)
    radius = float(map_config.radius)

    # round radius up to make it square-tileable
    radius = 2 ** math.ceil(math.log(radius * 120, 2)) / 120

    ((x1, y1), (x2, y2)) = centered_box(
        lon - min_lon, lat - min_lat, radius)
    (w, h) = (x2 - x1, y2 - y1)

    arr = band.ReadAsArray(xoff=x1, yoff=y1, win_xsize=w, win_ysize=h)

    population = np.maximum(arr, 0)

    if plot:
        plt.imshow(population)
        plt.show()

    water = -np.minimum(arr, 0)

    if plot:
        plt.imshow(water)
        plt.show()

    water_qtree = tile_water(water)

    write_qtree(state, water_qtree)

    if save is not None:
        state.save(save)


if __name__ == "__main__":
    try:
        # if invoked through bazel, use the natural working directory
        if "BUILD_WORKING_DIRECTORY" in os.environ:
            os.chdir(os.environ["BUILD_WORKING_DIRECTORY"])

        argh.dispatch_command(main)
    except KeyboardInterrupt:
        pass
