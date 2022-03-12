import typing as T

from collections import defaultdict
from functools import cached_property
from dataclasses import dataclass

from shapely.geometry import Point

import engine

from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate import osm


def parse_ref(ref: T.Optional[str]) -> T.List[str]:
    """
    Parses OSM's "ref" tag; splits into separate refs.
    """
    if ref is None:
        return None
    return ref.split(";")


def parse_lanes(lanes: T.Optional[str]) -> int:
    """
    Parse OSM's "lanes" tag; returns number of lanes.
    """
    if lanes is None:
        return None
    return int(lanes)


def parse_speed_limit(maxspeed: T.Optional[str]) -> int:
    """
    Parse OSM's "maxspeed" tag; returns speed limit in m/s.
    """
    if maxspeed is None:
        return None

    if maxspeed.endswith(" mph"):
        kph = float(maxspeed[:-4]) * 1.61
    else:
        kph = float(maxspeed)

    return round(kph / 3.6)


@dataclass
class SegmentData:
    name: T.Optional[str]
    ref: T.Optional[T.List[str]]
    lanes: T.Optional[int]
    speed_limit: T.Optional[int]


@dataclass
class Segment:
    id: int
    points: T.List[T.Tuple[float, float]]
    in_segments: T.List[int]  # list of ids of other Segments
    out_segments: T.List[int]  # list of ids of other Segments
    data: SegmentData


class Highways(Layer):
    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["osm"]

    @cached_property
    def max_depth(self) -> int:
        return self.map_config.engine_config["max_depth"]

    @cached_property
    def max_dim(self) -> int:
        return 2**self.max_depth

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData) -> None:
        assert False

    def round_coords(self, point):
        """
        Allows us to use a (float, float) pair as a key in a dictionary.
        Basically, rounding floats before comparing them allows for small
        discrepancies to be ignored.
        """
        if hasattr(point, "x") and hasattr(point, "y"):
            (x, y) = (point.x, point.y)
        else:
            (x, y) = point

        # round to 6 decimal places, which is way more precision than we need
        return (round(x, 6), round(y, 6))

    def post_init(self, dataset: osm.OsmData, qtree: Quadtree, state: T.Any) -> None:
        self.osm = dataset

        # each item is a (incoming, outgoing) tuple
        coord_map = defaultdict(lambda: ([], []))

        for highway in self.osm.highways:
            # NOTE: saw one case of a self-loop, which has no boundary
            if len(highway.shape.boundary.geoms) == 2:
                first, last = highway.shape.boundary.geoms
                coord_map[self.round_coords(first)][1].append(highway)
                coord_map[self.round_coords(last)][0].append(highway)

        junctions = []
        for (point, (in_ways, out_ways)) in coord_map.items():
            if len(in_ways) + len(out_ways) > 2:
                junctions.append((point, in_ways, out_ways))

        # NOTE: This approach will fail to detect closed loops. For
        # now, we'll say that this is OK. To add support for closed
        # loops, we can track visited points and then traverse any
        # unvisited points. We want to continue using this approach
        # because for segments, the starting point matters; but for
        # closed loops, it doesn't matter which point we start at.

        id_counter = 0
        segment_tuples = []
        segment_start_map = defaultdict(list)
        segment_end_map = defaultdict(list)

        def add_segment_tuple(start_way, end_way, points, segment_data):
            nonlocal id_counter

            if len(points) == 0:
                # this can happen if all of the points are outside the region of interest
                return

            start_pt, _ = start_way.shape.boundary.geoms
            _, end_pt = end_way.shape.boundary.geoms
            start = self.round_coords(start_pt)
            end = self.round_coords(end_pt)

            segment_tuples.append((id_counter, points, start, end, segment_data))
            segment_start_map[start].append(id_counter)
            segment_end_map[end].append(id_counter)

            id_counter += 1

        for (point, in_ways, out_ways) in junctions:
            # NOTE: only use diverging edges to avoid double-counting
            for highway in out_ways:
                points = []
                way = highway
                prev_segment_data = None
                while True:
                    cur_segment_data = SegmentData(
                        name=way.tags.get("name", None),
                        ref=parse_ref(way.tags.get("ref", None)),
                        lanes=parse_lanes(way.tags.get("lanes", None)),
                        speed_limit=parse_speed_limit(way.tags.get("maxspeed", None)),
                    )

                    # cut off segments that extend out of the region of interest
                    # TODO: split into two segments if this happens
                    for (x, y) in way.shape.coords:
                        if not (0 <= x <= self.max_dim and 0 <= y <= self.max_dim):
                            add_segment_tuple(highway, way, points, prev_segment_data)
                            points = []
                            break
                    else:
                        if (
                            prev_segment_data is not None
                            and prev_segment_data != cur_segment_data
                        ):
                            # if the properties of the segment have changed, create a new one
                            add_segment_tuple(highway, way, points, prev_segment_data)
                            points = []
                        points.extend(way.shape.coords)

                        prev_segment_data = cur_segment_data

                    _, last = way.shape.boundary.geoms
                    next_in_ways, next_out_ways = coord_map.get(self.round_coords(last))

                    if len(next_in_ways) != 1 or len(next_out_ways) != 1:
                        break
                    else:
                        # keep following the line
                        way = next_out_ways[0]

                if prev_segment_data is not None:
                    add_segment_tuple(highway, way, points, prev_segment_data)

        segments = []
        for (id, points, start, end, segment_data) in segment_tuples:
            segments.append(
                Segment(
                    id,
                    points,
                    segment_start_map[start],
                    segment_start_map[end],
                    segment_data,
                )
            )

        for segment in segments:
            data = engine.HighwayData(
                segment.data.name,
                segment.data.ref or [],
                segment.data.lanes,
                segment.data.speed_limit,
            )
            segment_id = state.add_highway_segment(
                data, segment.in_segments, segment.out_segments, segment.points
            )
            # NOTE: need to make sure IDs are mapped correctly.
            # this is gross, but as long as both this implementation and the Rust implementation
            # increment the IDs from zero, the IDs will match up and we don't need to worry
            assert segment_id == segment.id

    def merge(self, node: Quadtree, convolve: ConvolveData) -> None:
        pass

    def finalize(self, data: T.Any) -> Tile:
        raise UnimplementedError()

    def fuse(self, entities: T.List[T.Any]) -> T.Any:
        assert False, entities

    def modify_state(self, state: T.Any) -> None:
        pass
