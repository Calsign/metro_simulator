import typing as T

from collections import defaultdict
from functools import cached_property
from dataclasses import dataclass
from enum import Enum

from shapely.geometry import Point

from generate.common import parse_speed
from generate.data import MapConfig, round_coords
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData
from generate.network import Network, InputNode, InputWay

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


class RampDirection(Enum):
    ON_RAMP = 1
    OFF_RAMP = 2


@dataclass
class JunctionData:
    ramp: RampDirection


@dataclass
class SegmentData:
    name: T.Optional[str]
    ref: T.Optional[T.List[str]]
    lanes: T.Optional[int]
    speed_limit: T.Optional[int]


class Highways(Network):
    def __init__(self, map_config: MapConfig):
        super().__init__(map_config, has_way_segment_map=False)

    def get_ways(self, osm: osm.OsmData) -> T.Generator[InputWay, None, None]:
        for highway in self.osm.highways:
            highway_tag = highway.tags.get("highway")
            if highway_tag in ["motorway", "trunk"]:
                yield InputWay(
                    highway,
                    not is_oneway(highway.tags),
                    SegmentData(
                        name=highway.tags.get("name"),
                        ref=parse_ref(highway.tags),
                        lanes=parse_lanes(highway.tags),
                        speed_limit=parse_speed_limit(highway.tags),
                    ),
                )

    def get_nodes(self, osm: osm.OsmData) -> T.Generator[InputNode, None, None]:
        for highway in self.osm.highways:
            highway_tag = highway.tags.get("highway")
            if highway_tag in ["motorway_link", "trunk_link"]:
                endpoints = [round_coords(c) for c in highway.shape.boundary.geoms]
                if len(endpoints) == 0:
                    # TODO: this is confusing
                    continue
                # in general we might have an off-ramp at the start and an on-ramp at the end
                first, last = endpoints
                # TODO: currently this will attach both an off-ramp and an on-ramp, which is incorrect
                yield InputNode(first, 10, JunctionData(RampDirection.OFF_RAMP))
                yield InputNode(last, 10, JunctionData(RampDirection.ON_RAMP))

    def bake_junction(
        self, data: T.Optional[T.Any], state: T.Any, point: T.Tuple[float, float]
    ) -> T.Any:
        import engine

        (x, y) = point

        if data is None:
            ramp = None
        else:
            if data.ramp == RampDirection.ON_RAMP:
                ramp = engine.RampDirection.on_ramp()
            elif data.ramp == RampDirection.OFF_RAMP:
                ramp = engine.RampDirection.off_ramp()
            else:
                assert False, data.ramp

        return state.add_highway_junction(x, y, engine.HighwayJunctionData(ramp))

    def bake_segment(
        self,
        data: T.Optional[SegmentData],
        state: T.Any,
        start_id: T.Any,
        end_id: T.Any,
        points: T.List[T.Tuple[float, float]],
    ) -> T.Any:
        import engine

        assert data is not None

        data = engine.HighwaySegmentData(
            data.name,
            data.ref or [],
            data.lanes,
            data.speed_limit,
        )
        return state.add_highway_segment(data, start_id, end_id, points)

    def modify_state(self, state: T.Any, qtree: Quadtree) -> None:
        super().modify_state(state, qtree)

        if VALIDATE:
            state.validate_highways()
