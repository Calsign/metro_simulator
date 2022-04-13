from __future__ import annotations

import typing as T
import json
from dataclasses import dataclass
import functools

import shapely.geometry
from shapely.affinity import affine_transform

from generate.data import Coords, round_to_pow2, centered_box, EQ_KM_PER_DEG


class SupportsParse(T.Protocol):
    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> SupportsParse:
        pass


@dataclass
class Way:
    id: int
    tags: T.Dict[str, str]
    shape: T.Any

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Way:
        return Way(data["id"], data["tags"], shapely.geometry.shape(data["shape"]))

    def transform(self, matrix: T.List[float]):
        self.shape = affine_transform(self.shape, matrix)


@dataclass
class Node:
    id: int
    tags: T.Dict[str, str]
    location: T.Tuple[float, float]

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Node:
        return Node(**data)

    def transform(self, matrix: T.List[float]):
        p = shapely.geometry.Point(self.location)
        t = affine_transform(p, matrix)
        self.location = (t.x, t.y)


@dataclass
class RelMember:
    ref: int
    type: str
    role: str

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> RelMember:
        return RelMember(**data)

    def transform(self, matrix: T.List[float]):
        pass


@dataclass
class Relation:
    id: int
    tags: T.Dict[str, str]
    members: T.List[RelMember]

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Relation:
        return Relation(
            data["id"], data["tags"], list(map(RelMember.parse, data["members"]))
        )

    def transform(self, matrix: T.List[float]):
        for member in self.members:
            member.transform(matrix)


FIELDS: T.Dict[str, T.Type[SupportsParse]] = {
    "subways": Way,
    "stations": Node,
    "stops": Node,
    "subway_route_masters": Relation,
    "subway_routes": Relation,
    "highways": Way,
}


class OsmData:
    # NOTE: for mypy
    subways: T.List[Way]
    stations: T.List[Node]
    stops: T.List[Node]
    subway_route_masters: T.List[Relation]
    subway_routes: T.List[Relation]
    highways: T.List[Way]

    def __init__(self):
        for field in FIELDS:
            setattr(self, field, [])

    @functools.cached_property
    def subway_map(self) -> T.Dict[int, Way]:
        return {subway.id: subway for subway in self.subways}

    @functools.cached_property
    def stop_map(self) -> T.Dict[int, Node]:
        return {stop.id: stop for stop in self.stops}

    def transform(self, matrix: T.List[float]):
        for field in FIELDS:
            for item in getattr(self, field):
                item.transform(matrix)

    def plot_route(self, plt, route):
        color = route.tags.get("colour")
        for member in route.members:
            if (
                member.type == "w"
                and member.ref in self.subway_map
                and member.role == ""
            ):
                subway = self.subway_map[member.ref]
                plt.plot(*subway.shape.xy, color=color)

    def plot_highway(self, plt, highway):
        plt.plot(*highway.shape.xy, color="black")

    def plot(self, plt):
        reserved = ["all", "highways"]

        fig, axs = plt.subplots(
            len(reserved),
            figsize=(24, 24 * (len(reserved))),
        )

        axs[0].set_title("all")
        axs[1].set_title("highways")

        for i, route in enumerate(self.subway_routes):
            self.plot_route(axs[0], route)
            # self.plot_route(axs[i + len(reserved)], route)
            # axs[i + len(reserved)].set_title(route.tags.get("name", "<unnamed>"))

        for highway in self.highways:
            self.plot_highway(axs[1], highway)


def read_osm(dataset: T.Dict[str, T.Any], coords: Coords, max_dim: int) -> T.Any:
    (min_lon, max_lon) = (
        coords.lon - coords.lon_radius,
        coords.lon + coords.lon_radius,
    )
    (min_lat, max_lat) = (
        coords.lat - coords.lat_radius,
        coords.lat + coords.lat_radius,
    )

    # shape transformation matrix; applies translation and scaling to
    # translate from longitude/latitude to x/y coordinates
    xscale = max_dim / (max_lon - min_lon)
    yscale = max_dim / (max_lat - min_lat)
    matrix = [xscale, 0, 0, yscale, -min_lon * xscale, -min_lat * yscale]

    osm = OsmData()

    for path in sorted(dataset["tiles"]):
        with open(path, "r") as f:
            data = json.load(f)

        for field, cls in FIELDS.items():
            getattr(osm, field).extend([cls.parse(d) for d in data[field]])

    # sort to ensure hermeticity
    for field in FIELDS:
        getattr(osm, field).sort(key=lambda x: x.id)

    osm.transform(matrix)

    # rotate to correct orientation
    # TODO: figure out why this is necessary
    osm.transform([1, 0, 0, -1, 0, max_dim])

    return osm
