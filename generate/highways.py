import typing as T

from collections import defaultdict
from functools import cached_property
from dataclasses import dataclass

from shapely.geometry import Point

from generate.common import parse_speed
from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate import osm


# Whether or not to validate the constructed highways data structure.
# Best to leave disabled unless changing the logic here since it takes extra time.
VALIDATE = False


def parse_ref(tags: T.Dict[str, str]) -> T.Optional[T.List[str]]:
    """
    Parses OSM's "ref" tag; splits into separate refs.
    """
    ref = tags.get("ref")

    if ref is None:
        return None
    return ref.split(";")


def parse_lanes(tags: T.Dict[str, str]) -> T.Optional[int]:
    """
    Parse OSM's "lanes" tag; returns number of lanes.
    """
    lanes = tags.get("lanes")

    if lanes is None:
        forward = tags.get("lanes:forward")
        backward = tags.get("lanes:backward")
        if forward is not None and backward is not None:
            # slightly hacky, but does the trick
            lanes = "{};{}".format(forward, backward)
        else:
            return None

    try:
        total = sum([int(s.strip()) for s in lanes.split(";")])
        if total > 0:
            return total
        else:
            raise ValueError()
    except ValueError:
        print("Warning: failed to parse lanes: '{}'".format(lanes))
        return None


def parse_speed_limit(tags: T.Dict[str, str]) -> T.Optional[int]:
    """
    Parse OSM's "maxspeed" tag; returns speed limit in m/s.
    """
    maxspeed = tags.get("maxspeed")

    if maxspeed is None:
        return None

    try:
        return parse_speed(maxspeed)
    except ValueError:
        print("Warning: failed to parse maxspeed '{}'".format(maxspeed))
        return None


def is_oneway(tags: T.Dict[str, str]) -> bool:
    highway = tags["highway"]
    if highway == "motorway":
        # motorway implies oneway
        return tags.get("oneway", "yes").lower() not in ["no", "false", "0"] and not (
            "lanes:forward" in tags and "lanes:backward" in tags
        )
    else:
        # other highway tags default to bidirectional
        return tags.get("oneway", "no").lower() in ["yes", "true", "1"]


@dataclass
class SegmentData:
    name: T.Optional[str]
    ref: T.Optional[T.List[str]]
    lanes: T.Optional[int]
    speed_limit: T.Optional[int]


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

    def round_coords(self, point) -> T.Tuple[float, float]:
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

    def post_init(self, dataset: osm.OsmData, qtree: Quadtree) -> None:
        self.osm = dataset

    def merge(self, node: Quadtree, convolve: ConvolveData) -> None:
        pass

    def finalize(self, data: T.Any) -> Tile:
        raise NotImplementedError()

    def fuse(self, entities: T.List[T.Any]) -> T.Any:
        assert False, entities

    def modify_state(self, state: T.Any, qtree: Quadtree) -> None:
        import engine

        # each item is a (incoming, outgoing) tuple
        coord_map: T.Dict[
            T.Tuple[float, float], T.Tuple[T.List[osm.Way], T.List[osm.Way]]
        ] = defaultdict(lambda: ([], []))

        on_ramps = set()
        off_ramps = set()

        for highway in self.osm.highways:
            # NOTE: saw one case of a self-loop, which has no boundary
            if len(highway.shape.boundary.geoms) == 2:
                first, last = (
                    self.round_coords(c) for c in highway.shape.boundary.geoms
                )
                highway_tag = highway.tags.get("highway")
                if highway_tag in ["motorway", "trunk"]:
                    coord_map[first][1].append(highway)
                    coord_map[last][0].append(highway)
                    if not is_oneway(highway.tags):
                        # also add a segment in the opposite direction
                        coord_map[first][0].append(highway)
                        coord_map[last][1].append(highway)
                elif highway_tag in ["motorway_link", "trunk_link"]:
                    # in general we might have an off-ramp at the start and an on-ramp at the end
                    off_ramps.add(first)
                    on_ramps.add(last)
                else:
                    raise Exception("Unrecognized highway tag: {}".format(highway_tag))

        junctions = []
        for (point, (in_ways, out_ways)) in coord_map.items():
            if len(in_ways) + len(out_ways) != 2:
                # 3+ is a junction, 1 is a dead-end
                junctions.append((point, in_ways, out_ways))

        # NOTE: This approach will fail to detect closed loops. For
        # now, we'll say that this is OK. To add support for closed
        # loops, we can track visited points and then traverse any
        # unvisited points. We want to continue using this approach
        # because for segments, the starting point matters; but for
        # closed loops, it doesn't matter which point we start at.

        segment_tuples = []

        def add_segment_tuple(points, segment_data):
            if len(points) == 0:
                # this can happen if all of the points are outside the region of interest
                return

            start = self.round_coords(points[0])
            end = self.round_coords(points[-1])

            segment_tuples.append((points, start, end, segment_data))

        for (point, in_ways, out_ways) in junctions:
            # NOTE: only use diverging edges to avoid double-counting
            for highway in out_ways:
                points: T.List[T.Tuple[float, float]] = []
                way = highway
                border_point = point
                prev_segment_data = None

                while True:
                    cur_segment_data = SegmentData(
                        name=way.tags.get("name"),
                        ref=parse_ref(way.tags),
                        lanes=parse_lanes(way.tags),
                        speed_limit=parse_speed_limit(way.tags),
                    )

                    first, last = (
                        self.round_coords(c) for c in way.shape.boundary.geoms
                    )
                    if first == border_point:
                        # normal orientation
                        border_point = last
                        coords = way.shape.coords
                    elif last == border_point:
                        # flipped
                        border_point = first
                        coords = list(reversed(way.shape.coords))
                    else:
                        assert False, (border_point, first, last)

                    # cut off segments that extend out of the region of interest
                    # TODO: split into two segments if this happens
                    for (x, y) in coords:
                        if not (0 <= x <= self.max_dim and 0 <= y <= self.max_dim):
                            add_segment_tuple(points, prev_segment_data)
                            points = []
                            break
                    else:
                        if (
                            prev_segment_data is not None
                            and prev_segment_data != cur_segment_data
                        ):
                            # if the properties of the segment have changed, create a new one
                            add_segment_tuple(points, prev_segment_data)
                            points = []

                        points.extend(coords)
                        prev_segment_data = cur_segment_data

                    next_in_ways, next_out_ways = coord_map[border_point]

                    if len(next_in_ways) != 1 or len(next_out_ways) != 1:
                        break
                    else:
                        # keep following the line
                        way = next_out_ways[0]

                if prev_segment_data is not None:
                    add_segment_tuple(points, prev_segment_data)

        # map from points to junction IDs
        junction_map: T.Dict[T.Tuple[float, float], int] = {}

        def get_junction_id(point: T.Tuple[float, float]) -> int:
            if point in junction_map:
                return junction_map[point]
            else:
                (x, y) = point
                assert 0 <= x <= self.max_dim, (x, self.max_dim)
                assert 0 <= y <= self.max_dim, (y, self.max_dim)

                if point in on_ramps:
                    ramp = engine.RampDirection.on_ramp()
                elif point in off_ramps:
                    ramp = engine.RampDirection.off_ramp()
                else:
                    ramp = None

                junction_id = state.add_highway_junction(x, y, ramp)
                junction_map[point] = junction_id
                return junction_id

        for (points, start, end, segment_data) in segment_tuples:
            # create junctions if needed
            start_id = get_junction_id(start)
            end_id = get_junction_id(end)

            data = engine.HighwayData(
                segment_data.name,
                segment_data.ref or [],
                segment_data.lanes,
                segment_data.speed_limit,
            )
            segment_id = state.add_highway_segment(data, start_id, end_id, points)

        if VALIDATE:
            state.validate_highways()
