from __future__ import annotations

import typing as T
import json
from dataclasses import dataclass
import functools

import shapely.geometry
from shapely.affinity import affine_transform

from generate.data import Coords, round_to_pow2, centered_box, EQ_KM_PER_DEG


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


@dataclass
class OsmData:
    subways: T.List[Way]
    stations: T.List[Node]
    stops: T.List[Node]
    route_masters: T.List[Relation]
    routes: T.List[Relation]

    @staticmethod
    def create() -> OsmData:
        return OsmData([], [], [], [], [])

    @functools.cached_property
    def subway_map(self) -> T.Dict[int, Way]:
        return {subway.id: subway for subway in self.subways}

    @functools.cached_property
    def station_map(self) -> T.Dict[int, Node]:
        return {station.id: station for station in self.stations}

    @functools.cached_property
    def stop_map(self) -> T.Dict[int, Node]:
        return {stop.id: stop for stop in self.stops}

    @functools.cached_property
    def route_map(self) -> T.Dict[int, Relation]:
        return {route.id: route for route in self.routes}

    def transform(self, matrix: T.List[float]):
        for subway in self.subways:
            subway.transform(matrix)
        for station in self.stations:
            station.transform(matrix)
        for stop in self.stops:
            stop.transform(matrix)
        for route_master in self.route_masters:
            route_master.transform(matrix)
        for route in self.routes:
            route.transform(matrix)

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

    def plot(self, plt):
        fig, axs = plt.subplots(
            len(self.routes) + 1, figsize=(8, 8 * (len(self.routes) + 1))
        )

        axs[0].set_title("all")

        for i, route in enumerate(self.routes):
            self.plot_route(axs[0], route)
            self.plot_route(axs[i + 1], route)
            axs[i + 1].set_title(route.tags.get("name", "<unnamed>"))


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

    osm = OsmData.create()

    for path in sorted(dataset["tiles"]):
        with open(path, "r") as f:
            data = json.load(f)

        osm.subways.extend(map(Way.parse, data["subways"]))
        osm.stations.extend(map(Node.parse, data["stations"]))
        osm.stops.extend(map(Node.parse, data["stops"]))
        osm.route_masters.extend(map(Relation.parse, data["route_masters"]))
        osm.routes.extend(map(Relation.parse, data["routes"]))

    # sort to ensure hermeticity
    osm.subways.sort(key=lambda s: s.id)
    osm.stations.sort(key=lambda s: s.id)
    osm.stops.sort(key=lambda s: s.id)
    osm.route_masters.sort(key=lambda m: m.id)
    osm.routes.sort(key=lambda r: r.id)

    osm.transform(matrix)

    # rotate to correct orientation
    # TODO: figure out why this is necessary
    osm.transform([1, 0, 0, -1, 0, max_dim])

    return osm
