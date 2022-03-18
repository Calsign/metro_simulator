"""
Preprocessor for OpenStreetMap data from Geofabrik.

We split out this preprocessing step from the main generation because it takes
several minutes. This preprocessing pulls out all of the metro data in a
provided OSM pbf file and exports it to a json.
"""

import typing as T
import json

import argh
import osmium
import shapely.wkb
import shapely.geometry


class Handler(osmium.SimpleHandler):
    FIELDS = [
        "subways",
        "stations",
        "stops",
        "subway_route_masters",
        "subway_routes",
        "highways",
    ]

    def __init__(self):
        super().__init__()
        self.wkbfab = osmium.geom.WKBFactory()
        for field in self.FIELDS:
            setattr(self, field, [])

    def make_node(self, n: T.Any) -> T.Dict[str, str]:
        return {
            "id": n.id,
            "tags": dict(n.tags),
            "location": (n.location.lon, n.location.lat),
        }

    def make_way(self, w: T.Any) -> T.Dict[str, str]:
        shape = shapely.wkb.loads(self.wkbfab.create_linestring(w), hex=True)
        return {
            "id": w.id,
            "tags": dict(w.tags),
            "shape": shapely.geometry.mapping(shape),
        }

    def make_rel_member(self, m: T.Any):
        return {"ref": m.ref, "type": m.type, "role": m.role}

    def make_relation(self, r: T.Any) -> T.Dict[str, str]:
        return {
            "id": r.id,
            "tags": dict(r.tags),
            "members": [self.make_rel_member(m) for m in r.members],
        }

    def matches(self, tags: T.Dict[str, T.Any], **kwargs):
        """
        Returns true if each kwarg is a key-value pair in the provided tags
        dictionary. Used for matching multiple tags at the same time.
        """
        return all(tags.get(k) == v for k, v in kwargs.items())

    def way(self, w: T.Any):
        if self.matches(w.tags, railway="subway"):
            # https://wiki.openstreetmap.org/wiki/Tag:railway%3Dsubway
            self.subways.append(self.make_way(w))
        if any(
            self.matches(w.tags, highway=x)
            for x in ("motorway", "trunk", "motorway_link", "trunk_link")
        ):
            # https://wiki.openstreetmap.org/wiki/Tag:highway%3Dmotorway
            # https://wiki.openstreetmap.org/wiki/Tag:highway%3Dtrunk
            self.highways.append(self.make_way(w))

    def node(self, n: T.Any):
        if self.matches(n.tags, railway="station", station="subway"):
            # https://wiki.openstreetmap.org/wiki/Tag:railway%3Dstation
            self.stations.append(self.make_node(n))
        if self.matches(
            n.tags, railway="stop", public_transport="stop_position", subway="yes"
        ):
            # https://wiki.openstreetmap.org/wiki/Tag:public%20transport=stop%20position?uselang=en
            self.stops.append(self.make_node(n))

    def build_member(self, m: T.Any):
        return {"ref": m.ref, "type": m.type, "role": m.role}

    def relation(self, r: T.Any):
        if self.matches(r.tags, type="route_master", route_master="subway"):
            # https://wiki.openstreetmap.org/wiki/Relation:route_master
            self.subway_route_masters.append(self.make_relation(r))
        elif self.matches(r.tags, type="route", route="subway"):
            # https://wiki.openstreetmap.org/wiki/Tag:route%3Dsubway
            self.subway_routes.append(self.make_relation(r))

    def to_json(self):
        """
        Dump the collected metro data to a nested dict, suitable for
        serializing to json.
        """
        return {field: getattr(self, field) for field in self.FIELDS}


def main(path: str, output: str):
    """
    Entrypoint to preprocessor.

    :param path: path of the input OSM pbf file
    :param output: path to dump json to
    """

    handler = Handler()
    # NOTE: need to set locations=True to get locations, but it takes several
    # times longer.
    handler.apply_file(path, locations=True)

    with open(output, "w") as f:
        json.dump(handler.to_json(), f)


if __name__ == "__main__":
    argh.dispatch_command(main)
