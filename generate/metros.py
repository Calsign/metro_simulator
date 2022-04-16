import typing as T

from dataclasses import dataclass
from functools import cached_property, lru_cache
from enum import Enum

from shapely.geometry import LineString, MultiPoint, Point
from shapely.ops import nearest_points
from matplotlib.colors import to_rgba

import engine

from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate import osm


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


@lru_cache
def address_from_coords(x: int, y: int, max_depth: int) -> T.List[int]:
    max_dim = 2**max_depth

    min_x = 0
    max_x = max_dim
    min_y = 0
    max_y = max_dim

    quadrant_map = {
        (False, False): 0,
        (True, False): 1,
        (False, True): 2,
        (True, True): 3,
    }

    address = []

    for _ in range(max_depth):
        cx = (max_x + min_x) / 2
        cy = (max_y + min_y) / 2
        right = x >= cx
        bottom = y >= cy

        if right:
            min_x = cx
        else:
            max_x = cx

        if bottom:
            min_y = cy
        else:
            max_y = cy

        address.append(quadrant_map[(right, bottom)])

    return address


def round_station_location(loc: T.Tuple[float, float]) -> T.Tuple[int, int]:
    x, y = loc
    return (round(x - 0.5), round(y - 0.5))


@dataclass
class Station:
    id: int
    name: str
    location: T.Tuple[float, float]
    x: int
    y: int
    metro_lines: T.List[int]


@dataclass
class MetroLine:
    id: int
    name: str
    color: T.Tuple[int, int, int]
    keys: T.List[T.Any]
    stations: T.List[Station]


class Metros(Layer):
    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["osm"]

    @cached_property
    def max_depth(self) -> int:
        return self.map_config.engine_config["max_depth"]

    @cached_property
    def max_dim(self) -> int:
        return 2**self.max_depth

    @cached_property
    def metro_lines_stations(self) -> T.Tuple[T.List[MetroLine], T.List[Station]]:
        seen_routes = set()
        metro_lines = []

        stations: T.List[Station] = []
        stations_all_coords: T.List[T.Tuple[float, float]] = []
        stations_coord_map: T.Dict[T.Tuple[int, int], Station] = {}

        for station in self.osm.stations:
            name = station.tags.get("name")
            assert name is not None

            stations_all_coords.append(station.location)
            rx, ry = round_station_location(station.location)
            if 0 <= rx <= self.max_dim and 0 <= ry <= self.max_dim:
                st = Station(station.id, name, station.location, rx, ry, [])
                stations.append(st)
                stations_coord_map[(rx, ry)] = st

        for route in self.osm.subway_routes:
            # if we are crossing state boundaries, we have multiple copies of each route
            if route.id in seen_routes:
                continue
            seen_routes.add(route.id)

            name = route.tags.get("name")
            color = route.tags.get("colour")
            assert name is not None, route

            last_point = None
            stops: T.List[osm.Node] = []
            spline_all_coords: T.List[T.Tuple[float, float]] = []
            spline_coord_map: T.Dict[T.Tuple[float, float], int] = {}

            # pull out ways
            for member in route.members:
                if (
                    member.type == "w"
                    and member.ref in self.osm.subway_map
                    and member.role == ""
                ):
                    subway = self.osm.subway_map[member.ref]

                    if color is None:
                        color = subway.tags.get("colour")

                    first, last = subway.shape.boundary.geoms
                    if last_point is not None and last.distance(
                        last_point
                    ) < first.distance(last_point):
                        # the way is flipped around for some reason. need to correct it
                        last_point = first
                        coords = reversed(subway.shape.coords)
                    else:
                        last_point = last
                        coords = subway.shape.coords

                    for (x, y) in coords:
                        # discard out-of-bounds data
                        if 0 <= x <= self.max_dim and 0 <= y <= self.max_dim:
                            if (x, y) not in spline_coord_map:
                                spline_coord_map[(x, y)] = len(spline_all_coords)
                                spline_all_coords.append((x, y))
                elif (
                    member.type == "n"
                    and member.ref in self.osm.stop_map
                    and member.role == "stop"
                ):
                    stop = self.osm.stop_map[member.ref]
                    x, y = stop.location
                    if 0 <= x <= self.max_dim and 0 <= y <= self.max_dim:
                        stops.append(stop)

            keys: T.List[T.Any] = []

            for x, y in spline_all_coords:
                keys.append(engine.MetroKey.key(x, y))

            stations_multipoint = MultiPoint(stations_all_coords)
            spline_linestring = LineString(spline_all_coords)
            spline_multipoint = MultiPoint(spline_all_coords)

            to_insert = {}
            line_stations = []

            for stop in stops:
                # find the nearest point on the spline so that we can insert the stop
                loc = Point(stop.location)
                pt, _ = nearest_points(spline_multipoint, loc)
                index = spline_coord_map[pt.coords[0]]

                # Put stop into the correct slot between neighboring keys in the spline.
                # TODO: This is only relevant if the stop location isn't one of the key points.
                # For everything I've tested so far, the stop is also a key point, so this
                # is basically untested (and it's unclear if it is ever necessary with OSM data).
                prev = Point(spline_all_coords[index - 1])
                spline_pt, _ = nearest_points(spline_linestring, loc)
                if prev.distance(spline_pt) < prev.distance(pt):
                    index -= 1

                # find the nearest station so that we can associate this stop with the station
                station_pt, _ = nearest_points(stations_multipoint, loc)
                station_x, station_y = round_station_location(
                    (station_pt.x, station_pt.y)
                )
                station_address = address_from_coords(
                    station_x,
                    station_y,
                    self.max_depth,
                )
                station_data = stations_coord_map[(station_x, station_y)]
                line_stations.append(station_data)

                x, y = stop.location
                to_insert[index] = engine.MetroKey.stop(
                    x,
                    y,
                    engine.MetroStation(
                        station_data.name,
                        engine.Address(station_address, self.max_depth),
                    ),
                )

            # NOTE: traverse in reverse order so that we don't mess up the indices
            for index in reversed(sorted(to_insert)):
                keys.insert(index, to_insert[index])

            if color is None:
                print("Warning: missing color for metro line {}".format(name))
                color = "#000000"

            parsed_color = parse_color(color)

            # only add line if it has some in-bounds data
            if len(keys) > 0:
                metro_lines.append(
                    MetroLine(route.id, name, parsed_color, keys, line_stations)
                )

        return (metro_lines, stations)

    @property
    def metro_lines(self) -> T.List[MetroLine]:
        return self.metro_lines_stations[0]

    @property
    def stations(self) -> T.List[Station]:
        return self.metro_lines_stations[1]

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData) -> None:
        assert False

    def post_init(self, dataset: osm.OsmData, qtree: Quadtree, state: T.Any) -> None:
        self.osm = dataset

        # add metro lines
        for metro_line in self.metro_lines:
            line_id = state.add_metro_line(
                metro_line.name, metro_line.color, metro_line.keys
            )

            for station in metro_line.stations:
                station.metro_lines.append(line_id)

        # add stations
        for station in self.stations:
            x, y = map(round, station.location)
            address = address_from_coords(x, y, self.max_depth)

            child = qtree.get_or_create_child(address, lambda: ({}, None))
            # NOTE: higher priority than water
            self.set_node_data(child, [station], 110)

    def merge(self, node: Quadtree, convolve: ConvolveData) -> None:
        pass

    def finalize(self, data: Station) -> Tile:
        return Tile(
            "MetroStationTile",
            dict(name=data.name, x=data.x, y=data.y, ids=data.metro_lines),
        )

    def fuse(self, entities: T.List[Station]) -> Station:
        assert False, entities

    def modify_state(self, state: T.Any, qtree: Quadtree) -> None:
        pass
