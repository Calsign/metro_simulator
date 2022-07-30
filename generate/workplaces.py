import typing as T

from generate.data import MapConfig
from generate.simple_density import SimpleDensity


class Workplaces(SimpleDensity):
    def __init__(self, map_config: MapConfig):
        super().__init__(map_config, "WorkplaceTile")

    def get_dataset(self) -> T.Optional[T.Dict[str, T.Any]]:
        return self.map_config.datasets["employment"]
