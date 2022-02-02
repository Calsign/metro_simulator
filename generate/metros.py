import typing as T

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
        raise Exception("Unrecognized color format: {}".format(color))


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
        for route in self.osm.routes:
            name = route.tags.get("name")
            color = parse_color(route.tags.get("colour"))
            assert name is not None

            keys = []

            for member in route.members:
                if (
                    member.type == "w"
                    and member.ref in self.osm.subway_map
                    and member.role == ""
                ):
                    subway = self.osm.subway_map[member.ref]

                    for (x, y) in subway.shape.coords:
                        keys.append(engine.MetroKey.key(x, y))

            state.add_metro_line(name, color, keys)
