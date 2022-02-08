import typing as T

from dataclasses import dataclass
from functools import cached_property
from enum import Enum

from shapely.geometry import LineString, MultiPoint, Point
from shapely.ops import nearest_points
from matplotlib.colors import to_rgba

import engine

from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate.osm import OsmData, Station


def parse_color(color):
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
    def metro_lines(self):
        max_dim = 2 ** self.map_config.engine_config["max_depth"]

        seen_routes = set()
        metro_lines = []

        for route in self.osm.routes:
            # if we are crossing state boundaries, we have multiple copies of each route
            if route.id in seen_routes:
                continue
            seen_routes.add(route.id)

            name = route.tags.get("name")
            color = route.tags.get("colour")
            assert name is not None, route

            last_point = None
            stops: T.List[T.Tuple[osm.MetroStation]] = []
            all_coords: T.List[T.Tuple[float, float]] = []
            coord_map: T.Dict[T.Tuple[float, float], int] = {}

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

                    first, last = subway.shape.boundary
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
                        if 0 <= x <= max_dim and 0 <= y <= max_dim:
                            if (x, y) not in coord_map:
                                coord_map[(x, y)] = len(all_coords)
                                all_coords.append((x, y))
                elif (
                    member.type == "n"
                    and member.ref in self.osm.stop_map
                    and member.role == "stop"
                ):
                    stop = self.osm.stop_map[member.ref]
                    x, y = stop.location
                    if 0 <= x <= max_dim and 0 <= y <= max_dim:
                        stops.append(stop)

            keys: T.List[T.Any] = []

            for x, y in all_coords:
                keys.append(engine.MetroKey.key(x, y))

            full_spline = LineString(all_coords)
            multipoint = MultiPoint(all_coords)

            to_insert = {}

            for stop in stops:
                loc = Point(stop.location)
                pt, _ = nearest_points(multipoint, loc)
                index = coord_map[pt.coords[0]]

                # Put stop into the correct slot between neighboring keys in the spline.
                # TODO: This is only relevant if the stop location isn't one of the key points.
                # For everything I've tested so far, the stop is also a key point, so this
                # is basically untested (and it's unclear if it is ever necessary with OSM data).
                prev = Point(all_coords[index - 1])
                spline_pt, _ = nearest_points(full_spline, loc)
                if prev.distance(spline_pt) < prev.distance(pt):
                    index -= 1

                x, y = stop.location
                # TODO: correctly determine address
                to_insert[index] = engine.MetroKey.stop(
                    x, y, engine.MetroStation(engine.Address([]))
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
                metro_lines.append(MetroLine(route.id, name, parsed_color, keys, []))

        return metro_lines

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData):
        assert False

    def post_init(self, dataset: OsmData, qtree: Quadtree):
        self.osm = dataset

        # TODO: add metro stations to qtree
        pass

    def merge(self, node: Quadtree, convolve: ConvolveData):
        pass

    def finalize(self, data: T.Any) -> Tile:
        pass

    def fuse(self, entities: T.List[T.Any]) -> T.Any:
        pass

    def modify_state(self, state: T.Any):
        for metro_line in self.metro_lines:
            state.add_metro_line(metro_line.name, metro_line.color, metro_line.keys)
