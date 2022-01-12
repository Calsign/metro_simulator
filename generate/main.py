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
    latitude: str
    longitude: str
    radius: str

    engine_config: dict
    datasets: dict


@dataclass
class GeoTransform:
    lon_min: float
    lon_res: float
    lat_min: float
    lat_res: float

    @staticmethod
    def from_gdal(dataset):
        (lon_min, lon_res, _, lat_min, _, lat_res) = dataset.GetGeoTransform()
        return GeoTransform(lon_min, lon_res, lat_min, lat_res)


def round_to_sq(x):
    """
    Round up to the nearest perfect square integer.
    """
    return int(2 ** math.ceil(math.log(x, 2)))


def centered_box(lon, lat, radius, transform):
    assert -180 <= lon < 180, lon
    assert -90 <= lat <= 90, lat

    lon_px = math.floor((lon - transform.lon_min) / transform.lon_res)
    lat_px = math.floor((lat - transform.lat_min) / transform.lat_res)
    lon_rad = round_to_sq(radius / abs(transform.lon_res))
    lat_rad = round_to_sq(radius / abs(transform.lat_res))
    return ((lon_px - lon_rad, lat_px - lat_rad), (lon_px + lon_rad, lat_px + lat_rad))


def parse_lat_lon(lat, lon):
    assert lat[-1] in ["N", "S"]
    assert lon[-1] in ["W", "E"]

    latf = float(lat[:-1])
    lonf = float(lon[:-1])

    if lat[-1] == "S":
        latf *= -1
    if lon[-1] == "W":
        lonf *= -1

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


def main(map_path, plot=False, save=None):
    map_config = MapConfig(**toml.load(map_path))

    state = engine.State(engine.Config.from_json(
        json.dumps(map_config.engine_config)))

    gdal.UseExceptions()
    data = gdal.Open(map_config.datasets["population"], gdal.GA_ReadOnly)
    band = data.GetRasterBand(1)

    transform = GeoTransform.from_gdal(data)

    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)

    ((x1, y1), (x2, y2)) = centered_box(lon, lat, map_config.radius, transform)
    (w, h) = (x2 - x1, y2 - y1)

    arr = band.ReadAsArray(xoff=x1, yoff=y1, win_xsize=w, win_ysize=h)

    population = np.maximum(arr, 0)
    water = -np.minimum(arr, 0)

    water_qtree = tile_water(water)

    write_qtree(state, water_qtree)

    if save is not None:
        state.save(save)

    if plot:
        import matplotlib
        import matplotlib.pyplot as plt

        plt.imshow(population)
        plt.show()

        plt.imshow(water)
        plt.show()


if __name__ == "__main__":
    try:
        # if invoked through bazel, use the natural working directory
        if "BUILD_WORKING_DIRECTORY" in os.environ:
            os.chdir(os.environ["BUILD_WORKING_DIRECTORY"])

        argh.dispatch_command(main)
    except KeyboardInterrupt:
        pass
