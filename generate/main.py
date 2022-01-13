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

    qtree = Quadtree(max_depth=depth)
    qtree.fill(None)

    # population with population data from the input array
    def initial(node, data):
        if data.depth == depth:
            node.data = population_grid[data.x][data.y] / people_per_sim
            assert node.data >= 0
    qtree.convolve(initial)

    def divide(node, data):
        if node.data is not None and node.data >= 4:
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


def read_gdal(dataset_path, coords, band=1):
    data = gdal.Open(dataset_path, gdal.GA_ReadOnly)
    band = data.GetRasterBand(1)
    transform = GeoTransform.from_gdal(data)

    ((x1, y1), (x2, y2)) = centered_box(
        coords.lon, coords.lat, coords.lon_radius, coords.lat_radius, transform)
    (w, h) = (x2 - x1, y2 - y1)

    # let gdal take care of resampling for us
    downsampled_dim = round_to_pow2(h)
    return band.ReadAsArray(xoff=x1, yoff=y1, win_xsize=w, win_ysize=h,
                            buf_xsize=downsampled_dim, buf_ysize=downsampled_dim)


def handle_terrain(map_config, coords, plot):
    data = read_gdal(map_config.datasets["terrain"], coords)

    if plot:
        plt().imshow(data)
        plt().show()

    return tile_terrain(data)


def handle_housing(map_config, coords, plot):
    data = read_gdal(map_config.datasets["population"], coords)
    population = np.maximum(data, 0)

    if plot:
        plt().imshow(population)
        plt().show()

    return tile_housing(
        population, map_config.engine_config["people_per_sim"])


@argh.arg("--plot", action="append", type=str)
def main(map_path, plot=[], save=None):
    map_config = MapConfig(**toml.load(map_path))

    state = engine.State(engine.Config.from_json(
        json.dumps(map_config.engine_config)))

    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)
    radius = map_config.engine_config["min_tile_size"] * \
        2**map_config.engine_config["max_depth"] / 2
    coords = Coords(lat=lat, lon=lon, radius=radius)

    gdal.UseExceptions()

    terrain_qtree = handle_terrain(map_config, coords, "terrain" in plot)
    housing_qtree = handle_housing(map_config, coords, "housing" in plot)

    write_qtree(state, terrain_qtree)

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
