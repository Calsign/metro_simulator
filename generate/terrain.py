import typing as T

from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData


class Terrain(Layer):
    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["terrain"]

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData):
        # GlobCover represents water with 210
        if data == 210:
            # priority 100: water should take priority over everything else
            self.set_node_data(node, [True], 100)
        else:
            # priority -100: empty tiles should be replaced by everything else
            self.set_node_data(node, [False], -100)

    def post_init(self, dataset: T.Any, qtree: Quadtree, state: T.Any):
        pass

    def node_has_water(self, node: Quadtree) -> bool:
        return self.get_node_data(node)[0]

    def merge(self, node: Quadtree, convolve: ConvolveData):
        first = self.node_has_water(node.children[0])
        if all([self.node_has_water(c) == first for c in node.children]):
            for child in node.children:
                self.clear_node_data(child)
            self.set_node_data(node, [first], 100 if first else -100)

    def finalize(self, data: bool) -> Tile:
        if data:
            return Tile("WaterTile", {})
        else:
            return Tile("EmptyTile", {})

    def fuse(self, entities: T.List[bool]) -> bool:
        # should be impossible
        assert False, entities

    def modify_state(self, state):
        pass
