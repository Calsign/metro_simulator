from __future__ import annotations

import typing as T
import json
from dataclasses import dataclass
import functools

import shapely.geometry
from shapely.affinity import affine_transform

from generate.data import Coords, round_to_pow2, centered_box, EQ_KM_PER_DEG


@dataclass
class Subway:
    id: int
    tags: T.Dict[str, str]
    shape: T.Any

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Subway:
        return Subway(data["id"], data["tags"], shapely.geometry.shape(data["shape"]))

    def transform(self, matrix: T.List[float]):
        self.shape = affine_transform(self.shape, matrix)


@dataclass
class Station:
    id: int
    tags: T.Dict[str, str]
    location: T.Tuple[float, float]

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Station:
        return Station(**data)

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
class RouteMaster:
    id: int
    tags: T.Dict[str, str]
    members: T.List[RelMember]

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> RouteMaster:
        return RouteMaster(
            data["id"], data["tags"], list(map(RelMember.parse, data["members"]))
        )

    def transform(self, matrix: T.List[float]):
        for member in self.members:
            member.transform(matrix)


@dataclass
class Route:
    id: int
    tags: T.Dict[str, str]
    members: T.List[RelMember]

    @staticmethod
    def parse(data: T.Dict[str, T.Any]) -> Route:
        return Route(
            data["id"], data["tags"], list(map(RelMember.parse, data["members"]))
        )

    def transform(self, matrix: T.List[float]):
        for member in self.members:
            member.transform(matrix)


@dataclass
class OsmData:
    subways: T.List[Subway]
    stations: T.List[Station]
    route_masters: T.List[RouteMaster]
    routes: T.List[Route]

    @staticmethod
    def create() -> OsmData:
        return OsmData([], [], [], [])

    @functools.cached_property
    def subway_map(self) -> T.Dict[int, Subway]:
        return {subway.id: subway for subway in self.subways}

    @functools.cached_property
    def station_map(self) -> T.Dict[int, Station]:
        return {station.id: station for station in self.stations}

    @functools.cached_property
    def route_map(self) -> T.Dict[int, Route]:
        return {route.id: route for route in self.routes}

    def transform(self, matrix: T.List[float]):
        for subway in self.subways:
            subway.transform(matrix)
        for station in self.stations:
            station.transform(matrix)
        for route_master in self.route_masters:
            route_master.transform(matrix)
        for route in self.routes:
            route.transform(matrix)

    def plot(self, plt):
        for route in self.routes:
            color = route.tags.get("colour")
            for member in route.members:
                if (
                    member.type == "w"
                    and member.ref in self.subway_map
                    and member.role == ""
                ):
                    subway = self.subway_map[member.ref]
                    plt.plot(*subway.shape.xy, color=color)


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

    osm = OsmData([], [], [], [])

    for path in sorted(dataset["tiles"]):
        with open(path, "r") as f:
            data = json.load(f)

        osm.subways.extend(map(Subway.parse, data["subways"]))
        osm.stations.extend(map(Station.parse, data["stations"]))
        osm.route_masters.extend(map(RouteMaster.parse, data["route_masters"]))
        osm.routes.extend(map(Route.parse, data["routes"]))

    # sort to ensure hermeticity
    osm.subways.sort(key=lambda s: s.id)
    osm.stations.sort(key=lambda s: s.id)
    osm.route_masters.sort(key=lambda m: m.id)
    osm.routes.sort(key=lambda r: r.id)

    osm.transform(matrix)

    # rotate to correct orientation
    # TODO: figure out why this is necessary
    osm.transform([1, 0, 0, -1, 0, max_dim])

    return osm
