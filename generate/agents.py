import math
import typing as T

from generate.common import random
from generate.data import MapConfig
from generate.layer import Layer, Tile
from generate.quadtree import Quadtree, ConvolveData


class Agents(Layer):
    def __init__(self, map_config: MapConfig):
        super().__init__(map_config)

    def get_dataset(self) -> T.Optional[T.Dict[str, T.Any]]:
        return None

    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData):
        pass

    def post_init(self, dataset: T.Any, qtree: Quadtree):
        pass

    def merge(self, node: Quadtree, convolve: ConvolveData):
        pass

    def finalize(self, data: float) -> Tile:
        pass

    def fuse(self, entities: T.List[float]) -> float:
        pass

    def modify_state(self, state: T.Any, qtree: Quadtree):
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
                    for _ in range(node.data["tile"]["density"]):
                        housing.append(engine.Address(data.address, qtree.max_depth))
                elif t == "WorkplaceTile":
                    for _ in range(node.data["tile"]["density"]):
                        workplaces.append(engine.Address(data.address, qtree.max_depth))

        qtree.convolve(count_tiles)

        total_workers = min(len(housing), len(workplaces))
        print(f"housing: {len(housing)}, workplaces: {len(workplaces)}")
        print(f"adding {total_workers} working agents")

        rand = random(self.map_config.name)

        def create_agent():
            # TODO: generate ages and education levels from some data source
            birthday = engine.Date.from_ymd(2000, 1, 1)
            return engine.AgentData(birthday, 16)

        for _ in range(total_workers):
            housing_id = housing.pop(rand.randrange(0, len(housing)))
            workplace_id = workplaces.pop(rand.randrange(0, len(workplaces)))

            state.add_agent(create_agent(), housing_id, workplace_id)

        # If we have more housing than workplaces (which should normally be true), then add agents
        # without jobs. This includes not just unemployed people, but also people not working for
        # various other reasons, e.g. because they are children, retired, or stay-at-home parents.
        print(f"adding {len(housing)} non-working agents")
        for housing_id in housing:

            state.add_agent(create_agent(), housing_id, None)

        # TODO: add additional empty housing
