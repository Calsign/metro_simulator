import typing as T

import engine

from generate.common import random
from generate.data import MapConfig
from generate.quadtree import Quadtree
from generate.layer import Tile
from generate.simple_density import SimpleDensity


class Workplaces(SimpleDensity):
    def __init__(self, map_config: MapConfig):
        super().__init__(map_config, "WorkplaceTile")

    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["employment"]

    def modify_state(self, state: T.Any, qtree: Quadtree):
        # TODO: use LODES data to generate actual commutes
        # for now, we just assign commutes randomly
        housing = []
        workplaces = []

        def count_tiles(node, data):
            if isinstance(node.data, Tile):
                if node.data.kind == "HousingTile":
                    housing.append(data.address)
                elif node.data.kind == "WorkplaceTile":
                    workplaces.append(data.address)

        qtree.convolve(count_tiles)

        total_agents = min(len(housing), len(workplaces))

        rand = random(self.map_config.name)

        for _ in range(total_agents):
            housing_id = housing.pop(rand.randrange(0, len(housing)))
            workplace_id = workplaces.pop(rand.randrange(0, len(workplaces)))

            state.add_agent(engine.AgentData(), housing_id, workplace_id)
