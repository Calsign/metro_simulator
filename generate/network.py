from __future__ import annotations

import typing as T

from copy import deepcopy

from collections import defaultdict
from functools import cached_property
from dataclasses import dataclass
from functools import cached_property

from shapely.geometry import Point, MultiPoint, LineString, Point
from shapely.ops import nearest_points
from shapely.strtree import STRtree

from generate.common import parse_speed
from generate.data import MapConfig, round_coords
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate import osm


@dataclass
class InputNode:
    node: T.Union[osm.Node, T.Tuple[float, float]]
    max_dist: float  # how far away it can be from segments, in meters
    data: T.Any  # must be dataclass or similar type with structural equality


@dataclass
class InputWay:
    way: osm.Way
    bidirectional: bool
    data: T.Any  # must be dataclass or similar type with structural equality


@dataclass
class Segment:
    handle: T.Optional[T.Any]
    points: T.List[T.Tuple[float, float]]
    data: T.Any
    split: T.Optional[T.Tuple[Segment, Segment]]
    way_ids: T.List[int]

    @property
    def start(self) -> T.Any:
        return self.points[0]

    @property
    def end(self) -> T.Any:
        return self.points[-1]

    @cached_property
    def point_index_map(self) -> T.Mapping[T.Tuple[float, float], int]:
        return {round_coords(point): i for i, point in enumerate(self.points)}

    @cached_property
    def multipoint(self) -> T.Any:
        return MultiPoint(self.points)

    @cached_property
    def linestring(self) -> T.Any:
        return LineString(self.points)

    def endpoints(self) -> T.Tuple[T.Tuple[float, float], T.Tuple[float, float]]:
        return (self.start, self.end)

    def has_endpoint(self, endpoint: T.Tuple[float, float]) -> bool:
        return self.start == endpoint or self.end == endpoint


@dataclass
class Node:
    handle: T.Optional[T.Any]
    location: T.Tuple[float, float]
    data: T.Any


class Network(Layer):
    def __init__(self, map_config: MapConfig, has_way_segment_map=False):
        super().__init__(map_config)
        self.segments: T.List[Segment] = []
        self.nodes: T.Dict[T.Tuple[float, float], Node] = {}
        # map from way ids to the segments containing them
        self.way_segment_map: T.Dict[int, T.Set[int]] = defaultdict(set)
        self.id_segment_map: T.Dict[int, Segment] = {}
        # TODO: Currently way_segment_map is incompatible with bidirectional ways
        # because we assume that ways have unique ids. There are ways to address
        # this, but we don't need to yet because we only use bidirectional ways
        # for highways, and we only use way_segment_map for railways.
        self.has_way_segment_map = has_way_segment_map

    def get_ways(self, osm: osm.OsmData) -> T.Generator[InputWay, None, None]:
        """
        Yields ways which should be turned into segments.
        """
        raise NotImplementedError()

    def get_nodes(
        self, osm: osm.OsmData
    ) -> T.Generator[T.Union[InputNode], None, None]:
        """
        Yields nodes which should be turned into extra junctions. (Most junctions are implied by
        intersections of ways.)
        """
        raise NotImplementedError()

    def bake_junction(
        self, data: T.Optional[T.Any], state: T.Any, coords: T.Tuple[float, float]
    ) -> T.Any:
        raise NotImplementedError()

    def bake_segment(
        self,
        data: T.Any,
        state: T.Any,
        start_id: T.Any,
        end_id: T.Any,
        points: T.List[T.Tuple[float, float]],
    ) -> T.Any:
        raise NotImplementedError()

    def get_dataset(self) -> T.Optional[T.Dict[str, T.Any]]:
        return self.map_config.datasets["osm"]

    @cached_property
    def max_depth(self) -> int:
        return self.map_config.engine_config["max_depth"]

    @cached_property
    def max_dim(self) -> int:
        return 2**self.max_depth

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData) -> None:
        assert False

    def post_init(self, dataset: osm.OsmData, qtree: Quadtree) -> None:
        self.osm = dataset
        self.construct_data()

    def merge(self, node: Quadtree, convolve: ConvolveData) -> None:
        pass

    def finalize(self, data: T.Any) -> Tile:
        raise NotImplementedError()

    def fuse(self, entities: T.List[T.Any]) -> T.Any:
        assert False, entities

    def construct_data(self) -> None:
        # each item is a (incoming, outgoing) tuple
        coord_map: T.Dict[
            T.Tuple[float, float], T.Tuple[T.List[InputWay], T.List[InputWay]]
        ] = defaultdict(lambda: ([], []))

        for input_way in self.get_ways(self.osm):
            # NOTE: saw one case of a self-loop, which has no boundary
            if len(input_way.way.shape.boundary.geoms) != 2:
                continue

            first, last = (round_coords(c) for c in input_way.way.shape.boundary.geoms)
            coord_map[first][1].append(input_way)
            coord_map[last][0].append(input_way)
            if input_way.bidirectional:
                coord_map[first][0].append(input_way)
                coord_map[last][1].append(input_way)

        del input_way

        intersections = []
        for (point, (in_ways, out_ways)) in coord_map.items():
            if len(in_ways) != 1 or len(out_ways) != 1:
                # intersections and dead ends
                intersections.append((point, in_ways, out_ways))

        # NOTE: This approach will fail to detect closed loops. For
        # now, we'll say that this is OK. To add support for closed
        # loops, we can track visited points and then traverse any
        # unvisited points. We want to continue using this approach
        # because for segments, the starting point matters; but for
        # closed loops, it doesn't matter which point we start at.

        def add_segment(points, segment_data, way_ids):
            if len(points) == 0:
                # this can happen if all of the points are outside the region of interest
                return

            segment = Segment(None, points, segment_data, None, way_ids)

            if self.has_way_segment_map:
                for way_id in way_ids:
                    if way_id in self.way_segment_map:
                        # abandon ship; don't add to segments

                        # TODO: why does this happen?

                        # Sp far I've only seen this happen for one-way segments, but it's possible
                        # we are still fine with multi-way segments.
                        assert len(way_ids) == 1
                        return

                    self.way_segment_map[way_id].add(id(segment))
                    self.id_segment_map[id(segment)] = segment

            self.segments.append(segment)

        for (point, in_ways, out_ways) in intersections:
            # NOTE: only use diverging edges to avoid double-counting
            for out_way in out_ways:
                points: T.List[T.Tuple[float, float]] = []
                way_ids: T.List[int] = []
                way = out_way
                border_point = point
                prev_segment_data = None

                while True:
                    cur_segment_data = way.data

                    first, last = (
                        round_coords(c) for c in way.way.shape.boundary.geoms
                    )
                    # NOTE: for some reason ways are occasionally flipped, which breaks things
                    if first == border_point:
                        # normal orientation
                        border_point = last
                        coords = way.way.shape.coords
                    elif last == border_point:
                        # flipped
                        border_point = first
                        coords = list(reversed(way.way.shape.coords))
                    else:
                        assert False, (border_point, first, last)

                    # cut off segments that extend out of the region of interest
                    # TODO: split into two segments if this happens
                    for (x, y) in coords:
                        if not (0 <= x <= self.max_dim and 0 <= y <= self.max_dim):
                            add_segment(points, prev_segment_data, way_ids)
                            points = []
                            way_ids = []
                            break
                    else:
                        if (
                            prev_segment_data is not None
                            and prev_segment_data != cur_segment_data
                        ):
                            # if the properties of the segment have changed, create a new one
                            add_segment(points, prev_segment_data, way_ids)
                            points = []
                            way_ids = []

                        points.extend(coords)
                        way_ids.append(way.way.id)
                        prev_segment_data = cur_segment_data

                    next_in_ways, next_out_ways = coord_map[border_point]

                    if len(next_in_ways) != 1 or len(next_out_ways) != 1:
                        break
                    else:
                        # keep following the line
                        way = next_out_ways[0]

                if prev_segment_data is not None:
                    add_segment(points, prev_segment_data, way_ids)

        del point
        del points
        del way
        del coords

        junction_counts: T.Dict[T.Tuple[float, float], int] = defaultdict(lambda: 0)

        for segment in self.segments:
            junction_counts[round_coords(segment.start)] += 1
            junction_counts[round_coords(segment.end)] += 1

        # TODO: this doesn't seem to be working!
        def filter_segment(segment):
            # remove segments that are really short and don't connect to anything
            # TODO: remove short, disconnected sequences of segments
            if (
                junction_counts[segment.start] == 1
                and junction_counts[segment.end] == 1
                and segment.linestring().length < 2000
            ):
                return False

            return True

        self.segments = [
            segment for segment in self.segments if filter_segment(segment)
        ]

        del junction_counts
        del segment

        # Attempt to insert each node into an existing segment, splitting the segment in the
        # process.

        all_segments = []
        for segment in self.segments:
            shape = LineString(segment.points)
            all_segments.append(shape)

        del segment
        del shape

        str_tree = STRtree(all_segments)

        for input_node in self.get_nodes(self.osm):
            if isinstance(input_node.node, osm.Node):
                location = Point(input_node.node.location)
            else:
                location = Point(input_node.node)

            nearest_geometry = str_tree.nearest(location)
            nearest_segment = self.segments[nearest_geometry]

            # We may have already split this segment. We store a binary tree of segment splits which
            # we can traverse to find the leaf segment which should now be split.
            while nearest_segment.split:
                (a, b) = nearest_segment.split
                if location.distance(a.multipoint) < location.distance(b.multipoint):
                    nearest_segment = a
                else:
                    nearest_segment = b

            (nearest_point, _) = nearest_points(nearest_segment.multipoint, location)
            nearest_dist = location.distance(nearest_point)

            if (
                nearest_dist * self.map_config.engine_config["min_tile_size"]
                > input_node.max_dist
            ):
                continue

            nearest_point_tuple = (nearest_point.x, nearest_point.y)
            rounded_nearest_point = round_coords(nearest_point)

            # find index for insertion
            index = nearest_segment.point_index_map[rounded_nearest_point]

            # if at the end of a segment, no split is necessary
            if index > 0 and index < len(nearest_segment.points) - 1:
                prv = Point(nearest_segment.points[index - 1])
                if prv.distance(nearest_point) < nearest_dist:
                    index -= 1
                nxt = Point(nearest_segment.points[index + 1])
                if nxt.distance(nearest_point) < nearest_dist:
                    index += 1

                # NOTE: We treat split segments as encompassing all the ways of the parent. This is
                # OK since any metro line taking a single half of a split way will also take the
                # other half.
                # TODO: That isn't necessarily true! Metro lines can stop at stations and not keep going....

                a_points = nearest_segment.points[:index]
                if a_points[-1] != nearest_point_tuple:
                    a_points.append(nearest_point_tuple)

                b_points = nearest_segment.points[index:]
                if b_points[0] != nearest_point_tuple:
                    b_points.insert(0, nearest_point_tuple)

                # perform split
                a = Segment(
                    None,
                    a_points,
                    nearest_segment.data,
                    None,
                    nearest_segment.way_ids,
                )
                b = Segment(
                    None,
                    b_points,
                    nearest_segment.data,
                    None,
                    nearest_segment.way_ids,
                )
                nearest_segment.split = (a, b)

                if self.has_way_segment_map:
                    # reassign things in way_segment_map
                    for way_id in nearest_segment.way_ids:
                        if id(nearest_segment) in self.way_segment_map[way_id]:
                            self.way_segment_map[way_id].remove(id(nearest_segment))
                    for way_id in a.way_ids:
                        self.way_segment_map[way_id].add(id(a))
                    for way_id in b.way_ids:
                        self.way_segment_map[way_id].add(id(b))

                self.segments.append(a)
                self.segments.append(b)
                self.id_segment_map[id(a)] = a
                self.id_segment_map[id(b)] = b

            # TODO: a node might already exist at this point
            self.nodes[rounded_nearest_point] = Node(
                None, rounded_nearest_point, input_node.data
            )

    def modify_state(self, state: T.Any, qtree: Quadtree) -> None:
        import engine

        # map from points to junction IDs
        junction_map: T.Dict[T.Tuple[float, float], T.Any] = {}

        def get_junction_id(point: T.Tuple[float, float]) -> int:
            if point in junction_map:
                return junction_map[point]
            else:
                (x, y) = point
                assert 0 <= x <= self.max_dim, (x, self.max_dim)
                assert 0 <= y <= self.max_dim, (y, self.max_dim)

                node = self.nodes.get(round_coords(point), None)
                if node is None:
                    data = None
                else:
                    data = node.data

                junction_id = self.bake_junction(data, state, (x, y))
                junction_map[point] = junction_id
                if node is not None:
                    node.handle = junction_id
                return junction_id

        for segment in self.segments:
            if segment.split:
                continue

            # create junctions if needed
            start_id = get_junction_id(segment.start)
            end_id = get_junction_id(segment.end)

            segment.handle = self.bake_segment(
                segment.data, state, start_id, end_id, segment.points
            )
