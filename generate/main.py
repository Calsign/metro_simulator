#!/usr/bin/env python3

import os
import sys
import math
from dataclasses import dataclass
import json
import functools

import typing as T

import numpy as np
from osgeo import gdal
import toml
import argh

import engine

from quadtree import Quadtree


# Kilometers per degree at the equator
EQ_KM_PER_DEG = 111


@functools.lru_cache
def runfiles():
    from rules_python.python.runfiles import runfiles
    return runfiles.Create()


@functools.lru_cache
def plt():
    import matplotlib
    import matplotlib.pyplot as plt
    return plt


@dataclass
class MapConfig:
    latitude: str
    longitude: str

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


@dataclass
class Coords:
    lat: float
    lon: float
    radius: float  # meters

    @property
    def lon_radius(self):
        # account for curvature of the earth
        return self.radius / 1000 / EQ_KM_PER_DEG / \
            math.cos(math.radians(self.lat))

    @property
    def lat_radius(self):
        return self.radius / 1000 / EQ_KM_PER_DEG


class Plotter:
    def __init__(self, names_to_plot, plot_dir=None):
        self.names_to_plot = names_to_plot
        self.plot_dir = plot_dir
        self.plot_all = names_to_plot == ["all"]

    def plot(self, name, img):
        if self.plot_all or name in self.names_to_plot:
            plt().imshow(img)
            if self.plot_dir is not None:
                plt().savefig(os.path.join(self.plot_dir, "{}.png".format(name)))
            plt().show()


def round_to_pow2(x, up=True):
    """
    Round up or down to the nearest power of two.
    """
    f = (math.floor, math.ceil)[up]
    return int(2 ** f(math.log(x, 2)))


def centered_box(lon, lat, lon_radius, lat_radius, transform):
    assert -180 <= lon < 180, lon
    assert -90 <= lat <= 90, lat

    lon_px = math.floor((lon - transform.lon_min) / transform.lon_res)
    lat_px = math.floor((lat - transform.lat_min) / transform.lat_res)
    lon_rad = int(lon_radius / abs(transform.lon_res))
    lat_rad = int(lat_radius / abs(transform.lat_res))
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


def check_input_grid(grid):
    assert grid.shape[0] == grid.shape[1]
    dim = grid.shape[0]
    assert math.log(dim, 2) % 1 == 0, dim

    return (dim, int(math.log(dim, 2)))


def tile_terrain(terrain_grid):
    (dim, depth) = check_input_grid(terrain_grid)

    # NOTE: only handles water so far

    qtree = Quadtree(max_depth=depth)
    qtree.fill(None)

    # populate with water data from the input array
    def initial(node, data):
        if data.depth == depth:
            # GlobCover represents water as 210
            node.data = terrain_grid[data.x][data.y] == 210
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


def tile_housing(population_grid, people_per_sim):
    (dim, depth) = check_input_grid(population_grid)

    # TODO: fix max depth to allow splitting
    qtree = Quadtree(max_depth=depth)
    qtree.fill(None)

    # population with population data from the input array
    def initial(node, data):
        if data.depth == depth:
            node.data = population_grid[data.x][data.y] / people_per_sim
            if math.isnan(node.data):
                node.data = 0
            assert node.data >= 0, node.data
    qtree.convolve(initial)

    def divide(node, data):
        if node.data is not None and node.data >= 4 and data.depth < qtree.max_depth:
            for _ in range(4):
                node.add_child(node.data / 4)
    qtree.convolve(divide, post=False)

    def combine(node, data):
        if node.data is None:
            node.data = sum([child.data for child in node.children])
            if node.data < 4:
                # collapse small-population tiles together
                node.children = []
            else:
                # TODO: smart re-allocation of population
                pass
    qtree.convolve(combine, post=True)

    def convert(node, data):
        if len(node.children) == 0:
            density = round(node.data)
            if density == 0:
                node.data = make_tile("EmptyTile")
            else:
                node.data = make_tile("HousingTile", density=density)
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


def read_gdal(dataset: T.Dict[str, T.Any], coords: Coords, max_dim: int, band_num: int = 1):
    """
    Read data from a region of a (potentially tiled) dataset into a numpy array.

    Tiles must have the same resolution and cover the entire requested region.
    If tiles overlap, this function will not fail but the behavior is unspecified.

    :param dataset: a dataset; a dict with keys "tiles" (a list of paths to geotiff files)
                    and "data" (a dict with extra dataset metadata).
    :param coords: the coordinates of the region to load
    :param max_dim: the maximum width/height of the output array
    :param band_num: the GDAL band number to select
    """

    output = None
    lat_lon_res = None
    downsampled_dim = None

    total_area = 0

    # NOTE: sorted shouldn't be necessary, but for debugging it can be
    # useful for the results to be deterministic
    for data_file in sorted(dataset["tiles"]):
        data = gdal.Open(data_file, gdal.GA_ReadOnly)
        band = data.GetRasterBand(band_num)
        transform = GeoTransform.from_gdal(data)

        ((x1, y1), (x2, y2)) = centered_box(
            coords.lon, coords.lat, coords.lon_radius, coords.lat_radius, transform)

        current_lat_lon_res = (transform.lat_res, transform.lon_res)
        if output is None:
            # instantiate these values on the first pass because we need the resolution
            # this lets us load each file only once

            lat_lon_res = current_lat_lon_res

            downsample = dataset["data"]["downsample"]
            assert downsample >= 0
            downsampled_dim = min(round_to_pow2(y2 - y1), max_dim) \
                // (2 ** downsample)

            output = np.zeros([downsampled_dim, downsampled_dim])
        else:
            assert lat_lon_res == current_lat_lon_res, \
                "Got tiles with incompatible resolutions: {} != {}".format(
                    lat_lon_res, current_lat_lon_res)

        # crop to portion in this tile
        (x1c, y1c) = (min(max(x1, 0), band.XSize), min(max(y1, 0), band.YSize))
        (x2c, y2c) = (min(max(x2, 0), band.XSize), min(max(y2, 0), band.YSize))

        if x2c - x1c == 0 or y2c - y1c == 0:
            print("Unused dataset tile: {}".format(data_file))
        else:
            print("Using dataset tile: {}".format(data_file))

            # project portion of output covered by this tile into the output space
            (dx1, dy1) = (round((x1c - x1) / (x2 - x1) * downsampled_dim),
                          round((y1c - y1) / (y2 - y1) * downsampled_dim))
            (dx2, dy2) = (round((x2c - x1) / (x2 - x1) * downsampled_dim),
                          round((y2c - y1) / (y2 - y1) * downsampled_dim))

            # let gdal take care of resampling for us
            arr = band.ReadAsArray(xoff=x1c, yoff=y1c, win_xsize=x2c - x1c, win_ysize=y2c - y1c,
                                   buf_xsize=dx2 - dx1, buf_ysize=dy2 - dy1)
            output[dy1:dy2, dx1:dx2] = arr

            total_area += (dx2 - dx1) * (dy2 - dy1)

        # not necessary, but make clear that we no longer need this tile and it should be closed
        del data

    assert total_area >= downsampled_dim ** 2, \
        "Missing tiles, areas unequal: {} < {}".format(
            total_area, downsampled_dim ** 2)

    return output


def handle_terrain(map_config, coords, max_dim, plotter):
    data = read_gdal(map_config.datasets["terrain"], coords, max_dim)
    plotter.plot("terrain", data)
    return tile_terrain(data)


def handle_housing(map_config, coords, max_dim, plotter):
    data = read_gdal(map_config.datasets["population"], coords, max_dim)
    plotter.plot("housing", data)
    return tile_housing(data, map_config.engine_config["people_per_sim"])


@argh.arg("--plot", action="append", type=str)
def main(map_path, save=None, plot=[], plot_dir=None):
    if map_path.endswith(".toml"):
        map_config = MapConfig(**toml.load(map_path))
    elif map_path.endswith(".json"):
        with open(map_path) as f:
            map_config = MapConfig(**json.load(f))
    else:
        print("Unrecognized map file extension: {}".format(map_path))

    state = engine.State(engine.Config.from_json(
        json.dumps(map_config.engine_config)))

    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)
    radius = map_config.engine_config["min_tile_size"] * \
        2**map_config.engine_config["max_depth"] / 2
    coords = Coords(lat=lat, lon=lon, radius=radius)
    max_dim = 2 ** map_config.engine_config["max_depth"]

    gdal.UseExceptions()

    plotter = Plotter(plot, plot_dir)

    terrain_qtree = handle_terrain(map_config, coords, max_dim, plotter)
    housing_qtree = handle_housing(map_config, coords, max_dim, plotter)

    if save is not None:
        # TODO: merge qtrees
        write_qtree(state, housing_qtree)
        state.save(save)


if __name__ == "__main__":
    try:
        # if invoked through bazel, use the natural working directory
        if "BUILD_WORKING_DIRECTORY" in os.environ:
            os.chdir(os.environ["BUILD_WORKING_DIRECTORY"])

        argh.dispatch_command(main)
    except KeyboardInterrupt:
        pass
