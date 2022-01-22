from abc import ABC, abstractmethod
from dataclasses import dataclass
import typing as T

from quadtree import Quadtree, ConvolveData


@dataclass
class Tile:
    kind: str
    fields: T.Dict[str, T.Any]

    def to_json(self):
        return {
            "tile": {
                "type": self.kind,
                **self.fields,
            }
        }


class Layer(ABC):
    def __init__(self, map_config: T.Dict[str, T.Any]):
        self.map_config = map_config

    def get_node_data(self, node: Quadtree) -> T.List[T.Any]:
        """
        Return all data associated with this layer in the given node.
        """
        if self.get_name() in node.data[0]:
            return node.data[0][self.get_name()][0]
        else:
            return []

    def set_node_data(self, node: Quadtree, data: T.List[T.Any], priority: int):
        """
        Set the data associated with this layer in the given node.

        :param node: the given node
        :param data: list of individual data entities
        :param priority: higher priorities override data produced by other layers
        """
        assert priority is not None

        if node.data[1] is not None:
            # Maintain extra data invariants.
            # TODO: handle priorities too
            extra = node.data[1]
            if self.get_name() in node.data[0]:
                extra.total_entities -= len(node.data[0][self.get_name()][0])
            extra.total_entities += len(data)

        node.data[0][self.get_name()] = (data, priority)

    def clear_node_data(self, node: Quadtree):
        """
        Clear the data associated with this layer in the given node.
        """

        if node.data[1] is not None:
            # Maintain extra data invariants.
            # TODO: handle priorities too
            extra = node.data[1]
            if self.get_name() in node.data[0]:
                extra.total_entities -= len(node.data[0][self.get_name()][0])

        node.data[0][self.get_name()] = ([], None)

    @classmethod
    def get_name(cls) -> str:
        return cls.__name__.lower()

    @abstractmethod
    def get_dataset(self) -> T.Dict[str, T.Any]:
        """
        Return the dataset used by this layer.
        Should fetch it from self.map_config.datasets.
        """
        raise NotImplementedError

    @abstractmethod
    def initialize(self, data: int, node: Quadtree, convolve: ConvolveData):
        """
        Initialize the given node based on the provided data from the dataset.
        Call self.set_node_data to do this.
        """
        raise NotImplementedError

    @abstractmethod
    def merge(self, node: Quadtree, convolve: ConvolveData):
        """
        Optionally merge data from the given nodes children into the node.
        Use self.get_node_data on the children and self.set_node_data on the node.
        """
        raise NotImplementedError

    @abstractmethod
    def finalize(self, data: T.Any) -> Tile:
        """
        Return the tile corresponding to the given data entity.
        This is called once each node in the quadtree is assigned to a specific
        layer data entity.
        """
        raise NotImplementedError

    def fuse(self, entities: T.List[T.Any]) -> T.Any:
        """
        Fuse multiple entities into one entity. The implementation is allowed
        to just pick one if they cannot be fused. This function gets called
        when the quadtrree cannot be divided any further.
        """
        raise NotImplementedError
