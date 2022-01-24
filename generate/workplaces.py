import typing as T

from simple_density import SimpleDensity


class Workplaces(SimpleDensity):
    def __init__(self, map_config: T.Dict[str, T.Any]):
        super().__init__(map_config, "WorkplaceTile")

    def get_dataset(self) -> T.Dict[str, T.Any]:
        return self.map_config.datasets["employment"]
