from __future__ import annotations

import typing as T

from dataclasses import dataclass
from functools import cached_property
from enum import Enum
from collections import defaultdict

from shapely.geometry import LineString, MultiPoint, Point
from shapely.ops import nearest_points
from matplotlib.colors import to_rgba

from generate.common import parse_speed
from generate.data import MapConfig, address_from_coords
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData
from generate.network import Network, InputNode, InputWay, Segment
from generate.highways import parse_speed_limit

from generate import osm


class BrokenMetroLineException(Exception):
    def __init__(
        self,
        metro_line: osm.Relation,
        route_index: int,
        segment_set_index: int,
        all_prev_endpoints: T.List[T.Tuple[float, float]],
        prev_endpoints: T.List[T.Tuple[float, float]],
        prev_segments: T.List[Segment],
        current_segments: T.List[Segment],
        linearized_segments: T.List[Segment],
    ) -> None:
        super().__init__(
            f"""
Broken metro line. Details:

Metro line: {metro_line}

Route index: {route_index}
Segment set index: {segment_set_index}

All previous endpoints: {all_prev_endpoints}
Previous endpoints: {prev_endpoints}
Previous segments: {prev_segments}

Current segments: {current_segments}
Linearized segments: {linearized_segments}
        """
        )


class HandleBrokenMetroLineStrategy(Enum):
    FAIL = 1
    SKIP = 2

    @staticmethod
    def default() -> HandleBrokenMetroLineStrategy:
        HandleBrokenMetroLineStrategy.FAIL


def parse_color(color: str):
    if color.startswith("#") and len(color) == 7:
        r = int(color[1:3], 16)
        g = int(color[3:5], 16)
        b = int(color[5:7], 16)
        return (r, g, b)
    else:
        try:
            (r, g, b, _) = to_rgba(color)
            return (int(r * 255), int(g * 255), int(b * 255))
        except ValueError:
            raise Exception("Unrecognized color: {}".format(color))


def round_station_location(loc: T.Tuple[float, float]) -> T.Tuple[int, int]:
    x, y = loc
    return (round(x - 0.5), round(y - 0.5))


@dataclass
class Station:
    name: str
    location: T.Tuple[float, float]
    x: int
    y: int


@dataclass
class JunctionData:
    station: Station


@dataclass
class SegmentData:
    speed_limit: T.Optional[int]


@dataclass
class Schedule:
    fixed_frequency: int


@dataclass
class MetroLineData:
    name: str
    color: T.Tuple[int, int, int]
    schedule: Schedule
    speed_limit: int


@dataclass
class MetroLine:
    data: MetroLineData
    segments: T.List[Segment]


class Metros(Network):
    def __init__(self, map_config: MapConfig):
        super().__init__(map_config, has_way_segment_map=True)
        self.metro_lines: T.List[MetroLine] = []

    @cached_property
    def speed_limits(self) -> T.Dict[str, int]:
        dataset = self.get_dataset()
        assert dataset is not None
        return {
            network: parse_speed(speed_limit)
            for network, speed_limit in dataset["data"]["subway_speeds"].items()
        }

    @cached_property
    def broken_metro_line_strategies(self) -> T.Dict[str, str]:
        dataset = self.get_dataset()
        assert dataset is not None
        strategies = defaultdict(HandleBrokenMetroLineStrategy.default)
        for network, strategy in dataset["data"][
            "broken_metro_line_strategies"
        ].items():
            strategies[network] = HandleBrokenMetroLineStrategy[strategy.upper()]
        return strategies

    @cached_property
    def stations(self) -> T.List[Station]:
        stations = []

        for station in self.osm.stations:
            name = station.tags.get("name")
            if name is None:
                print("Warning: station {} is missing name".format(station.id))
                name = ""

            (rx, ry) = round_station_location(station.location)

            if 0 <= rx < self.max_dim and 0 <= ry < self.max_dim:
                stations.append(Station(name, station.location, rx, ry))

        return stations

    def get_ways(self, osm: osm.OsmData) -> T.Generator[InputWay, None, None]:
        for subway in self.osm.subways:
            # already filtered by the preprocessor
            yield InputWay(
                subway,
                False,
                SegmentData(parse_speed_limit(subway.tags)),
            )

    def get_nodes(self, osm: osm.OsmData) -> T.Generator[InputNode, None, None]:
        station_map = {(station.x, station.y): station for station in self.stations}
        stations_multipoint = MultiPoint(
            [(station.x, station.y) for station in self.stations]
        )

        for stop in osm.stops:
            (nearest_point, _) = nearest_points(
                stations_multipoint, Point(stop.location)
            )
            nearest_xy = (nearest_point.x, nearest_point.y)
            assert nearest_xy in station_map
            station = station_map[nearest_xy]
            yield InputNode(stop.location, 100, JunctionData(station))

    def bake_junction(
        self, data: T.Optional[T.Any], state: T.Any, point: T.Tuple[float, float]
    ) -> T.Any:
        import engine

        (x, y) = point

        if data is None:
            station = None
        else:
            address = address_from_coords(
                data.station.x, data.station.y, self.max_depth
            )
            station = engine.Station(
                data.station.name, engine.Address(address, self.max_depth)
            )

        return state.add_railway_junction(x, y, engine.RailwayJunctionData(station))

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

        data = engine.RailwaySegmentData(data.speed_limit)
        return state.add_railway_segment(data, start_id, end_id, points)

    def post_init_route(self, route_index: int, route: osm.Relation) -> None:
        # TODO: instead of assuming the OSM way ordering is reliable, find a good ordering of
        # the segments with the correct orientation
        segment_sets: T.List[T.Set[int]] = []

        for member in route.members:
            if (
                member.type == "w"
                and member.ref in self.osm.subway_map
                and member.role == ""
            ):
                way = self.osm.subway_map[member.ref]
                if way.id not in self.way_segment_map:
                    # this can happen if the way is out of bounds
                    continue

                way_segments = self.way_segment_map[way.id]
                if len(segment_sets) == 0 or way_segments != segment_sets[-1]:
                    segment_sets.append(way_segments)

            elif (
                member.type == "n"
                and member.ref in self.osm.stop_map
                and member.role == "stop"
            ):
                pass

        if len(segment_sets) == 0:
            return

        del member
        del way
        del way_segments

        segments: T.List[Segment] = []

        # rectify segment sets into segments
        for (i, segment_set) in enumerate(segment_sets):
            assert len(segment_set) > 0

            # linearize
            junctions: T.Mapping[T.Tuple[float, float], T.List[Segment]] = defaultdict(
                list
            )
            for segment_id in segment_set:
                segment = self.id_segment_map[segment_id]
                junctions[segment.start].append(segment)
                junctions[segment.end].append(segment)

            del segment_id
            del segment

            endpoints: T.List[T.Tuple[float, float]] = []
            for (point, segs) in junctions.items():
                assert len(segs) in (1, 2), (len(segs), segs)
                if len(segs) == 1:
                    endpoints.append(point)

            assert len(endpoints) == 2, (
                endpoints,
                [self.id_segment_map[seg] for seg in segment_set],
            )

            del point
            del segs

            current_point = endpoints[0]
            linearized_segments = [junctions[current_point][0]]
            while True:
                next_points = [
                    endpoint
                    for endpoint in linearized_segments[-1].endpoints()
                    if endpoint != current_point
                ]
                assert len(next_points) == 1
                next_point = next_points[0]
                next_junction = junctions[next_point]
                assert len(next_junction) in (1, 2)
                next_segments = [
                    seg for seg in next_junction if seg != linearized_segments[-1]
                ]
                if len(next_segments) == 0:
                    # reached the end
                    assert next_point in endpoints, (current_point, endpoints)
                    break
                next_segment = next_segments[0]
                linearized_segments.append(next_segment)
                current_point = next_point

                del next_points
                del next_point
                del next_junction
                del next_segments
                del next_segment

            assert len(linearized_segments) == len(segment_set), (
                "metro line",
                route.tags,
                "metro line index",
                route_index,
                "index",
                i,
                "endpoints",
                endpoints,
                "linearized segments",
                linearized_segments,
                "segment set",
                [self.id_segment_map[seg] for seg in segment_set],
            )

            del endpoints

            def check(segment: Segment, linearized_segment: Segment) -> bool:
                return segment.has_endpoint(
                    linearized_segment.start
                ) or segment.has_endpoint(linearized_segment.end)

            if len(segments) == 0:
                # First one; don't have a reference point, so just add them. If we get the
                # orientation wrong, we will need to fix it later.
                segments.extend(linearized_segments)
            else:
                # find correct orientation

                if check(segments[-1], linearized_segments[0]):
                    segments.extend(linearized_segments)
                elif check(segments[-1], linearized_segments[-1]):
                    segments.extend(reversed(linearized_segments))

                # we may have picked the wrong orientation for the first set
                elif i == 1 and check(segments[0], linearized_segments[0]):
                    segments.reverse()
                    segments.extend(linearized_segments)
                elif i == 1 and check(segments[0], linearized_segments[-1]):
                    segments.reverse()
                    segments.extend(reversed(linearized_segments))

                # we might be at a turnaround
                # the longest this can be is a single pre-split segment, i.e. a segment set
                elif i >= 1 and check(
                    segments[-len(segment_sets[i - 1])], linearized_segments[0]
                ):
                    segments.extend(reversed(segments[-len(segment_sets[i - 1]) : -1]))
                    segments.extend(linearized_segments)
                elif i >= 1 and check(
                    segments[-len(segment_sets[i - 1])], linearized_segments[-1]
                ):
                    segments.extend(reversed(segments[-len(segment_sets[i - 1]) : -1]))
                    segments.extend(reversed(linearized_segments))

                elif (
                    self.broken_metro_line_strategies[route.tags["network"]]
                    == HandleBrokenMetroLineStrategy.SKIP
                ):
                    # sure thing, let's just skip it
                    print(
                        "Skipping broken metro line: {}".format(route.tags.get("name"))
                    )
                    return

                else:
                    raise BrokenMetroLineException(
                        metro_line=route,
                        route_index=route_index,
                        segment_set_index=i,
                        all_prev_endpoints=[
                            segment.endpoints() for segment in segments
                        ],
                        prev_endpoints=segments[-1].endpoints(),
                        prev_segments=[
                            self.id_segment_map[seg] for seg in segment_sets[i - 1]
                        ],
                        current_segments=[
                            self.id_segment_map[seg] for seg in segment_set
                        ],
                        linearized_segments=linearized_segments,
                    )

            del segment_set
            del linearized_segments

        if len(segments) == 0:
            return

        del segment_sets

        name = route.tags.get("name")
        assert name is not None, route

        color = route.tags.get("colour")
        if color is None:
            print("Warning: missing color for metro line {}".format(name))
            color = "#000000"
        parsed_color = parse_color(color)
        # TODO: generate schedules
        schedule = Schedule(60 * 15)

        metro_network = route.tags["network"]
        if metro_network not in self.speed_limits:
            raise Exception(
                "Missing speed limit for metro network {}".format(metro_network)
            )
        speed_limit = self.speed_limits[metro_network]

        data = MetroLineData(name, parsed_color, schedule, speed_limit)
        self.metro_lines.append(MetroLine(data, segments))

    def post_init(self, dataset: osm.OsmData, qtree: Quadtree) -> None:
        super().post_init(dataset, qtree)

        for station in self.stations:
            address = address_from_coords(station.x, station.y, self.max_depth)
            child = qtree.get_or_create_child(address, lambda: ({}, None))
            # NOTE: higher priority than water
            self.set_node_data(child, [station], 110)

        del station
        del address
        del child

        seen_routes = set()
        for (route_index, route) in enumerate(self.osm.subway_routes):
            # if we are crossing state boundaries, we have multiple copies of each route
            if route.id in seen_routes:
                continue
            seen_routes.add(route.id)

            self.post_init_route(route_index, route)

    def merge(self, node: Quadtree, convolve: ConvolveData) -> None:
        pass

    def finalize(self, data: Station) -> Tile:
        return Tile(
            "MetroStationTile",
            dict(
                name=data.name,
                x=data.x,
                y=data.y,
                # TODO: set metro lines
                ids=[],
            ),
        )

    def fuse(self, entities: T.List[Station]) -> Station:
        assert False, entities

    def modify_state(self, state: T.Any, qtree: Quadtree) -> None:
        super().modify_state(state, qtree)

        import engine

        for metro_line in self.metro_lines:
            segment_handles = []
            for segment in metro_line.segments:
                assert segment.handle is not None
                segment_handles.append(segment.handle)

            state.add_metro_line(
                engine.MetroLineData(
                    metro_line.data.color,
                    metro_line.data.name,
                    engine.Schedule.fixed_frequency(
                        metro_line.data.schedule.fixed_frequency
                    ),
                    metro_line.data.speed_limit,
                ),
                segment_handles,
            )

        state.validate_metro_lines()
