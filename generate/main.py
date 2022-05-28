#!/usr/bin/env python3

import os
import sys
import math
import time
from dataclasses import dataclass
from collections import defaultdict
import json
import functools
import pickle
import importlib

import typing as T

import numpy as np

import toml
import argh

from generate.common import random
from generate.quadtree import Quadtree, ConvolveData
from generate.layer import Layer, Tile

from generate.data import Coords, MapConfig, round_to_pow2, centered_box
from generate.gdal import read_gdal
from generate.lodes import read_lodes
from generate.osm import read_osm

from generate import terrain, housing, workplaces, metros, highways, agents


LAYERS: T.List[T.Type[Layer]] = [
    terrain.Terrain,
    housing.Housing,
    workplaces.Workplaces,
    metros.Metros,
    highways.Highways,
    agents.Agents,
]


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

    def plot(self, name, data):
        if self.plot_all or name in self.names_to_plot:
            p = plt()
            p.clf()

            if hasattr(data, "plot"):
                data.plot(p)
            else:
                # assume it is an image/nparray
                p.imshow(data)

            if self.plot_dir is not None:
                p.savefig(os.path.join(self.plot_dir, "{}.png".format(name)))
            p.show()


@dataclass
class BakerData:
    """
    Data passed from the generator to the baker.
    """

    layers: T.List[Layer]
    map_config: MapConfig
    qtree: Quadtree


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
    import engine

    def write(node, data):
        address = engine.Address(data.address, qtree.max_depth)
        if len(node.children) > 0:
            assert len(node.children) == 4
            state.split(
                address,
                engine.BranchState(),
                engine.LeafState(),
                engine.LeafState(),
                engine.LeafState(),
                engine.LeafState(),
            )
        else:
            dumped = json.dumps(node.data)
            try:
                state.set_leaf_json(address, dumped)
            except Exception as e:
                print("Dumped json: {}".format(dumped))
                raise e

    qtree.convolve(write)


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
def generate(
    map_path,
    output_path=None,
    plot=[],
    plot_dir=None,
    profile_file=None,
    clean_script=None,
) -> None:
    if map_path.endswith(".toml"):
        map_config = MapConfig(**toml.load(map_path))
    elif map_path.endswith(".json"):
        with open(map_path) as mf:
            map_config = MapConfig(**json.load(mf))
    else:
        print("Unrecognized map file extension: {}".format(map_path))

    if profile_file is not None:
        profile().enable()

    report_timestamp("start")

    max_depth = map_config.engine_config["max_depth"]
    (lat, lon) = parse_lat_lon(map_config.latitude, map_config.longitude)
    radius = map_config.engine_config["min_tile_size"] * 2**max_depth / 2
    coords = Coords(lat=lat, lon=lon, radius=radius)
    max_dim = 2**max_depth

    plotter = Plotter(plot, plot_dir)

    layers = [layer(map_config) for layer in LAYERS]
    layer_map = {layer.get_name(): layer for layer in layers}

    qtree = Quadtree(max_depth=max_depth)

    if clean_script is not None:
        report_timestamp("load cleaner script")
        cleaner = importlib.import_module(clean_script)
    else:
        cleaner = None

    for layer in layers:
        report_timestamp("read dataset - {}".format(layer.get_name()))

        dataset_info = layer.get_dataset()
        if dataset_info is None:
            dataset_type = None
        else:
            dataset_type = dataset_info["data"]["type"]

        if dataset_type == "geotiff":
            dataset = read_gdal(dataset_info, coords, max_dim)
        elif dataset_type == "lodes":
            dataset = read_lodes(dataset_info, coords, max_dim)
        elif dataset_type == "open_street_map":
            dataset = read_osm(dataset_info, coords, max_dim)
        elif dataset_type is None:
            dataset = None
        else:
            raise Exception("Unrecognized dataset type: {}".format(dataset_type))

        if cleaner is not None and hasattr(cleaner, layer.get_name()):
            report_timestamp("cleaning - {}".format(layer.get_name()))
            getattr(cleaner, layer.get_name())(dataset)

        report_timestamp("plot - {}".format(layer.get_name()))
        plotter.plot(layer.get_name(), dataset)

        if isinstance(dataset, np.ndarray):
            (dim, depth) = check_input_grid(dataset)
            tile_width = max_dim // dim

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

        report_timestamp("post-init - {}".format(layer.get_name()))

        layer.post_init(dataset, qtree)

    if output_path is not None or profile_file is not None:
        # remove all entities in children with lower priority than the highest parent entity priority
        priority_stack: T.List[int] = []

        def bubble_priority_down(node, convolve):
            nonlocal priority_stack

            if node.data is not None:
                max_priority = max_or_none(
                    priority for (_, priority) in node.data[0].values()
                )
            else:
                max_priority = None

            for _ in range(len(priority_stack) - convolve.depth):
                priority_stack.pop()

            if len(priority_stack) > 0:
                current_priority = max_or_none((max_priority, priority_stack[-1]))
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
                    if all(
                        layer.get_name() in child.data[0] for child in node.children
                    ):
                        layer.merge(node, convolve)

            # mark nodes with minimum/maximum priorities of all entities that they contain
            # and remove child nodes with no entities

            min_child_priority = min_or_none(
                child.data[1].min_priority for child in node.children
            )
            max_child_priority = max_or_none(
                child.data[1].min_priority for child in node.children
            )
            child_entities = sum(
                child.data[1].total_entities for child in node.children
            )
            assert (min_child_priority is None) == (max_child_priority is None)

            if child_entities == 0:
                # if children have no entities, then get rid of the children
                node.children.clear()

            if node.data is not None:
                min_priority = min_or_none(
                    (
                        min_or_none(
                            priority for (_, priority) in node.data[0].values()
                        ),
                        min_child_priority,
                    )
                )
                max_priority = max_or_none(
                    (
                        max_or_none(
                            priority for (_, priority) in node.data[0].values()
                        ),
                        max_child_priority,
                    )
                )
                total_entities = child_entities + sum(
                    len(entities) for (entities, _) in node.data[0].values()
                )
                node_data = node.data[0]
            else:
                min_priority = None
                max_priority = None
                total_entities = 0
                node_data = {}

            assert (min_priority is None) == (max_priority is None)

            node.data = (
                node_data,
                NodeExtra(
                    min_priority=min_priority,
                    max_priority=max_priority,
                    total_entities=total_entities,
                ),
            )

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
                    node.add_children(
                        lambda: (
                            {},
                            NodeExtra(
                                min_priority=None, max_priority=None, total_entities=0
                            ),
                        )
                    )

                # sort in increasing priority
                all_entities.sort(key=lambda triple: triple[2])

                # divide entities among children
                for (layer, entity, priority) in all_entities:
                    if priority is None:
                        assert False, all_entities

                    # only place in a child with low enough minimum priority
                    possible_children = list(
                        filter(
                            lambda child: (child.data[1].min_priority or -math.inf)
                            <= priority,
                            node.children,
                        )
                    )

                    if len(possible_children) == 0:
                        # can't propagate this entitity down so it's gone
                        continue

                    # prioritize putting in children with fewer total entities first
                    min_total_entities = min(
                        child.data[1].total_entities for child in possible_children
                    )
                    minimal_children = list(
                        filter(
                            lambda child: child.data[1].total_entities
                            == min_total_entities,
                            possible_children,
                        )
                    )

                    assert len(minimal_children) > 0, (
                        possible_children,
                        minimal_children,
                    )

                    # select random child from the possible children
                    # NOTE: this random selection is deterministic because it
                    # uses a deterministic seed
                    child = minimal_children[
                        random(map_config.name).randrange(0, len(minimal_children))
                    ]

                    if layer not in child.data[0] or child.data[0][layer][1] is None:
                        # need to assign a priority
                        child.data[0][layer] = ([entity], priority)
                    else:
                        # TODO: we are dropping the priority here, which could matter
                        child.data[0][layer][0].append(entity)

                    # maintain extra data in child
                    child.data[1].total_entities += 1
                    child.data[1].max_priority = max_or_none(
                        (child.data[1].max_priority, priority)
                    )
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

        if output_path is not None:
            baker_data = BakerData(layers, map_config, qtree)
            with open(output_path, "wb") as of:
                pickle.dump(baker_data, of)

    if profile_file is not None:
        profile().disable()
        profile().dump_stats(profile_file)

    report_timestamp("done")


def bake(baker_data_path: str, output_path: str) -> None:
    import engine

    with open(baker_data_path, "rb") as f:
        baker_data = pickle.load(f)

    state = engine.Engine(
        engine.Config.from_json(json.dumps(baker_data.map_config.engine_config))
    )

    report_timestamp("write qtree")
    write_qtree(state, baker_data.qtree)

    for layer in baker_data.layers:
        report_timestamp("modify state - {}".format(layer.get_name()))
        layer.modify_state(state, baker_data.qtree)

    report_timestamp("save")
    state.save(output_path)


if __name__ == "__main__":
    try:
        # if invoked through bazel, use the natural working directory
        if "BUILD_WORKING_DIRECTORY" in os.environ:
            os.chdir(os.environ["BUILD_WORKING_DIRECTORY"])

        argh.dispatch_commands([generate, bake])
    except KeyboardInterrupt:
        pass
