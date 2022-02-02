import typing as T
import json

import argh
import osmium
import shapely.wkb
import shapely.geometry


class Handler(osmium.SimpleHandler):
    def __init__(self):
        super().__init__()

        self.wkbfab = osmium.geom.WKBFactory()

        self.subways = []
        self.stations = []
        self.route_masters = []
        self.routes = []

    def matches(self, tags, **kwargs):
        return all(tags.get(k) == v for k, v in kwargs.items())

    def way(self, w):
        if self.matches(w.tags, railway="subway"):
            shape = shapely.wkb.loads(self.wkbfab.create_linestring(w), hex=True)
            self.subways.append(
                {
                    "id": w.id,
                    "tags": dict(w.tags),
                    "shape": shapely.geometry.mapping(shape),
                }
            )

    def node(self, n):
        if self.matches(n.tags, railway="station", station="subway"):
            self.stations.append(
                {
                    "id": n.id,
                    "tags": dict(n.tags),
                    "location": (n.location.lon, n.location.lat),
                }
            )

    def build_member(self, m):
        return {"ref": m.ref, "type": m.type, "role": m.role}

    def relation(self, r):
        if self.matches(r.tags, type="route_master", route_master="subway"):
            self.route_masters.append(
                {
                    "id": r.id,
                    "tags": dict(r.tags),
                    "members": [self.build_member(m) for m in r.members],
                }
            )
        elif self.matches(r.tags, type="route", route="subway"):
            self.routes.append(
                {
                    "id": r.id,
                    "tags": dict(r.tags),
                    "members": [self.build_member(m) for m in r.members],
                }
            )

    def to_json(self):
        return {
            "subways": self.subways,
            "stations": self.stations,
            "route_masters": self.route_masters,
            "routes": self.routes,
        }


def main(path: str, output: str):
    handler = Handler()
    handler.apply_file(path, locations=True)

    with open(output, "w") as f:
        json.dump(handler.to_json(), f)


if __name__ == "__main__":
    argh.dispatch_command(main)
