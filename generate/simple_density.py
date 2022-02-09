import math
import typing as T

from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData


class SimpleDensity(Layer):
    def __init__(self, map_config: MapConfig, tile_name: str):
        super().__init__(map_config)
        self.tile_name = tile_name

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData):
        if math.isnan(data):
            data = 0
        assert data >= 0, data

        # convert real people units to simulated people units
        data /= self.map_config.engine_config["people_per_sim"]

        if data == 0:
            self.clear_node_data(node)
        elif data > 1:
            # potentially subdivide into many small tiles
            units = math.floor(data)
            per_unit = data / units
            self.set_node_data(node, [per_unit for _ in range(units)], 0)
        else:
            # most likely need to merge with neighboring tiles
            self.set_node_data(node, [data], 0)

    def post_init(self, dataset: T.Any, qtree: Quadtree, state: T.Any):
        pass

    def merge(self, node: Quadtree, convolve: ConvolveData):
        total = sum(sum(self.get_node_data(child)) for child in node.children)
        if total == 0:
            # no people here. carry on.
            self.clear_node_data(node)
        elif total < 4:
            # TODO: perhaps preserve existing distribution better
            for child in node.children:
                self.clear_node_data(child)

            units = max(math.floor(total), 1)
            per_unit = total / units
            self.set_node_data(node, [per_unit for _ in range(units)], 0)

    def finalize(self, data: float) -> Tile:
        if 0.2 < data < 1:
            # be conservative: make tiles in some situations that we wouldn't otherwise
            # this makes a more spatially diverse map
            rounded = 1
        else:
            rounded = round(data)
        if rounded == 0:
            return Tile("EmptyTile", {})
        else:
            return Tile(self.tile_name, {"density": rounded})

    def fuse(self, entities: T.List[float]) -> float:
        return sum(entities)

    def modify_state(self, state):
        pass
