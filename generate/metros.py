import typing as T

from matplotlib.colors import to_rgba

import engine

from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData

from generate.osm import OsmData


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


class Metros(Layer):
    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["osm"]

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
        max_dim = 2 ** self.map_config.engine_config["max_depth"]

        for route in self.osm.routes:
            name = route.tags.get("name")
            color = route.tags.get("colour")
            assert name is not None, route

            keys: T.List[T.Any] = []
            last_point = None

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
                    if len(keys) > 1 and last.distance(last_point) < first.distance(
                        last_point
                    ):
                        # the way is flipped around for some reason. need to correct it
                        last_point = first
                        coords = reversed(subway.shape.coords)
                    else:
                        last_point = last
                        coords = subway.shape.coords

                    for (x, y) in coords:
                        # discard out-of-bounds data
                        if 0 <= x <= max_dim or 0 <= y <= max_dim:
                            keys.append(engine.MetroKey.key(x, y))

            if color is None:
                print("Warning: missing color for metro line {}".format(name))
                color = "#000000"

            parsed_color = parse_color(color)

            # only add line if it has no out-of-bounds data
            if len(keys) > 0:
                state.add_metro_line(name, parsed_color, keys)
