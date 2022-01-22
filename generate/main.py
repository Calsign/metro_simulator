#!/usr/bin/env python3

import os
import sys
import math
import time
from dataclasses import dataclass
from collections import defaultdict
import json
import functools

import typing as T

import numpy as np
from osgeo import gdal
import toml
import argh

import engine

from quadtree import Quadtree, ConvolveData
from layer import Layer, Tile

import terrain
import housing


# Kilometers per degree at the equator
EQ_KM_PER_DEG = 111

LAYERS = [terrain.Terrain, housing.Housing]


@functools.lru_cache
def runfiles():
    from rules_python.python.runfiles import runfiles
    return runfiles.Create()


@functools.lru_cache
def plt():
    import matplotlib
    import matplotlib.pyplot as plt
    return plt


@functools.lru_cache
def profile():
    import cProfile
    return cProfile.Profile()


@functools.lru_cache
def random(seed):
    # NOTE: as long as we use a deterministic seed, the sequence of random
    # numbers will be deterministic. This is important for hermeticity.

    # NOTE: if we ever want to parallelize the generation, it will be important
    # to consider how the random number sequence is affected.
    import random
    return random.Random(seed)


@dataclass
class MapConfig:
    name: str

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


@dataclass
class NodeExtra:
    min_priority: T.Optional[int]
    max_priority: T.Optional[int]
    total_entities: int


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


def check_input_grid(grid):
    assert grid.shape[0] == grid.shape[1]
    dim = grid.shape[0]
    assert math.log(dim, 2) % 1 == 0, dim

    return (dim, int(math.log(dim, 2)))


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


@functools.lru_cache
def start_time():
    return time.time()


def report_timestamp(name):
    diff = round(time.time() - start_time(), 3)
    print("{}: {}".format(diff, name))


def most_or_none(vals, op):
    # TODO: this is really slow.
    most = None
    for val in vals:
        if val is not None:
            if most is None:
                most = val
            else:
                most = op(val, most)
    return most


def max_or_none(vals):
    return most_or_none(vals, max)


def min_or_none(vals):
    return most_or_none(vals, min)


@argh.arg("--plot", action="append", type=str)
def main(map_path, save=None, plot=[], plot_dir=None, profile_file=None):
    if map_path.endswith(".toml"):
        map_config = MapConfig(**toml.load(map_path))
    elif map_path.endswith(".json"):
        with open(map_path) as f:
            map_config = MapConfig(**json.load(f))
    else:
        print("Unrecognized map file extension: {}".format(map_path))

    if profile_file is not None:
        profile().enable()

    report_timestamp("start")

    state = engine.State(engine.Config.from_json(
        json.dumps(map_config.engine_config)))

    max_depth = map_config.engine_config["max_depth"]
    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)
    radius = map_config.engine_config["min_tile_size"] * 2 ** max_depth / 2
    coords = Coords(lat=lat, lon=lon, radius=radius)
    max_dim = 2 ** max_depth

    gdal.UseExceptions()

    plotter = Plotter(plot, plot_dir)

    layers = [layer(map_config) for layer in LAYERS]
    layer_map = {layer.get_name(): layer for layer in layers}

    qtree = Quadtree(max_depth=max_depth)

    for layer in layers:
        report_timestamp("read gdal - {}".format(layer.get_name()))
        dataset = read_gdal(layer.get_dataset(), coords, max_dim)
        (dim, depth) = check_input_grid(dataset)
        tile_width = max_dim // dim

        report_timestamp("plot - {}".format(layer.get_name()))
        plotter.plot(layer.get_name(), dataset)

        report_timestamp("fill - {}".format(layer.get_name()))
        qtree.fill(lambda: ({}, None), depth)

        def initialize(node, convolve):
            if convolve.depth == depth:
                x = convolve.x // tile_width
                y = convolve.y // tile_width
                data = dataset[x][y]
                layer.initialize(data, node, convolve)

        report_timestamp("initialize - {}".format(layer.get_name()))
        qtree.convolve(initialize)

    if save is not None or profile_file is not None:
        # remove all entities in children with lower priority than the highest parent entity priority
        priority_stack = []

        def bubble_priority_down(node, convolve):
            nonlocal priority_stack

            max_priority = max_or_none(priority
                                       for (_, priority) in node.data[0].values())

            for _ in range(len(priority_stack) - convolve.depth):
                priority_stack.pop()

            if len(priority_stack) > 0:
                current_priority = max_or_none(
                    (max_priority, priority_stack[-1]))
            else:
                current_priority = max_priority
            priority_stack.append(current_priority)

            if current_priority is not None:
                to_remove = []
                for (layer, (entities, priority)) in node.data[0].items():
                    if len(entities) > 0 and priority < current_priority:
                        to_remove.append(layer)
                for layer in to_remove:
                    del node.data[0][layer]

        report_timestamp("bubble priority down")
        qtree.convolve(bubble_priority_down, post=False)

        def merge(node, convolve):
            if len(node.children) > 0:
                for layer in layers:
                    if all(layer.get_name() in child.data[0] for child in node.children):
                        layer.merge(node, convolve)

            # mark nodes with minimum/maximum priorities of all entities that they contain
            # and remove child nodes with no entities

            min_child_priority = min_or_none(child.data[1].min_priority
                                             for child in node.children)
            max_child_priority = max_or_none(child.data[1].min_priority
                                             for child in node.children)
            child_entities = sum(child.data[1].total_entities
                                 for child in node.children)
            assert (min_child_priority is None) == (max_child_priority is None)

            if child_entities == 0:
                # if children have no entities, then get rid of the children
                node.children.clear()

            min_priority = min_or_none((
                min_or_none(priority
                            for (_, priority) in node.data[0].values()),
                min_child_priority,
            ))
            max_priority = max_or_none((
                max_or_none(priority
                            for (_, priority) in node.data[0].values()),
                max_child_priority,
            ))
            total_entities = child_entities + \
                sum(len(entities) for (entities, _) in node.data[0].values())
            assert (min_priority is None) == (max_priority is None)

            node.data = (node.data[0], NodeExtra(
                min_priority=min_priority,
                max_priority=max_priority,
                total_entities=total_entities
            ))

        report_timestamp("merge")
        qtree.convolve(merge, post=True)

        empty_tile_json = Tile("EmptyTile", {}).to_json()

        def split(node, convolve):
            all_entities = []
            for (layer, (entities, priority)) in node.data[0].items():
                for entity in entities:
                    all_entities.append((layer, entity, priority))

            if len(all_entities) == 1 and len(node.children) == 0:
                # finalize this node
                (layer, entity, _) = all_entities[0]
                node.data = layer_map[layer].finalize(entity).to_json()
            elif len(all_entities) == 0 and len(node.children) == 0:
                # finalize this empty node
                node.data = empty_tile_json
            elif len(all_entities) == 0:
                # we can safely do nothing
                pass
            elif convolve.depth < qtree.max_depth:
                # this node will have children, so it should not have any entities
                extra = node.data[1]
                node.data = None

                # create children if needed
                if len(node.children) == 0:
                    node.add_children(lambda: ({}, NodeExtra(
                        min_priority=None, max_priority=None, total_entities=0)))

                # sort in increasing priority
                all_entities.sort(key=lambda triple: triple[2])

                # divide entities among children
                for (layer, entity, priority) in all_entities:
                    if priority is None:
                        assert False, all_entities

                    # only place in a child with low enough minimum priority
                    possible_children = list(filter(
                        lambda child: (
                            child.data[1].min_priority or -math.inf) <= priority,
                        node.children))

                    if len(possible_children) == 0:
                        # can't propagate this entitity down so it's gone
                        continue

                    # prioritize putting in children with fewer total entities first
                    min_total_entities = min(
                        child.data[1].total_entities for child in possible_children)
                    minimal_children = list(filter(
                        lambda child: child.data[1].total_entities == min_total_entities,
                        possible_children))

                    assert len(
                        minimal_children) > 0, (possible_children, minimal_children)

                    # select random child from the possible children
                    # NOTE: this random selection is deterministic because it
                    # uses a deterministic seed
                    child = minimal_children[random(
                        map_config.name).randrange(0, len(minimal_children))]

                    if layer not in child.data[0] or child.data[0][layer][1] is None:
                        # need to assign a priority
                        child.data[0][layer] = ([entity], priority)
                    else:
                        # TODO: we are dropping the priority here, which could matter
                        child.data[0][layer][0].append(entity)

                    # maintain extra data in child
                    child.data[1].total_entities += 1
                    child.data[1].max_priority = max_or_none((
                        child.data[1].max_priority, priority))
            else:
                # splitting would exceed maximum depth; need to pick an entity

                # find the layer with the most entities
                all_layers = defaultdict(lambda: 0)
                layer_entities = defaultdict(list)
                for (layer, entity, _) in all_entities:
                    all_layers[layer] += 1
                    layer_entities[layer].append(entity)

                most_layer = max(all_layers, key=all_layers.get)

                # fuse all entities from the layer with the most entities
                fused = layer_map[most_layer].fuse(layer_entities[most_layer])
                node.data = layer_map[most_layer].finalize(fused).to_json()

        report_timestamp("split")
        qtree.convolve(split, post=False)

        report_timestamp("write qtree")
        write_qtree(state, qtree)

        if save is not None:
            report_timestamp("save")
            state.save(save)

    if profile_file is not None:
        profile().disable()
        profile().dump_stats(profile_file)

    report_timestamp("done")


if __name__ == "__main__":
    try:
        # if invoked through bazel, use the natural working directory
        if "BUILD_WORKING_DIRECTORY" in os.environ:
            os.chdir(os.environ["BUILD_WORKING_DIRECTORY"])

        argh.dispatch_command(main)
    except KeyboardInterrupt:
        pass
