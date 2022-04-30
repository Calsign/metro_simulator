import typing as T

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
        super().modify_state(state, qtree)

        import engine

        # TODO: use LODES data to generate actual commutes
        # for now, we just assign commutes randomly
        housing = []
        workplaces = []

        def count_tiles(node, data):
            if (
                node.data is not None
                and "tile" in node.data
                and "type" in node.data["tile"]
            ):
                t = node.data["tile"]["type"]
                if t == "HousingTile":
                    housing.append(engine.Address(data.address, qtree.max_depth))
                elif t == "WorkplaceTile":
                    workplaces.append(engine.Address(data.address, qtree.max_depth))

        qtree.convolve(count_tiles)

        total_agents = min(len(housing), len(workplaces))
        print(f"housing: {len(housing)}, workplaces: {len(workplaces)}")
        print(f"adding {total_agents} agents")

        rand = random(self.map_config.name)

        # TODO: set agents on housing/workplace tiles

        for _ in range(total_agents):
            housing_id = housing.pop(rand.randrange(0, len(housing)))
            workplace_id = workplaces.pop(rand.randrange(0, len(workplaces)))

            state.add_agent(engine.AgentData(), housing_id, workplace_id)
