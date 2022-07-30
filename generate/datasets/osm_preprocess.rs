//! A tool for extracting the data we care about from Geofabrik OSM extracts. There's a lot of data,
//! so it needs to be fast. Luckily, this tool is blazingly fast, especially compared to the
//! previous Python implementation.

use std::collections::HashSet;
use std::fs::File;
use std::path::PathBuf;

use osmpbfreader::objects::{
    Node, NodeId, OsmId, OsmObj, Ref, Relation, RelationId, Tags, Way, WayId,
};
use osmpbfreader::OsmPbfReader;

#[derive(clap::Parser, Debug)]
struct Args {
    path: PathBuf,
    output: PathBuf,
    patches: Vec<PathBuf>,
}

trait ContainsAny {
    fn any_value<'a, I: IntoIterator<Item = &'a str>>(&self, key: &str, values: I) -> bool;
    fn any_key<'a, I: IntoIterator<Item = &'a str>>(&self, keys: I, value: &str) -> bool;
}

impl ContainsAny for Tags {
    fn any_value<'a, I: IntoIterator<Item = &'a str>>(&self, key: &str, values: I) -> bool {
        values.into_iter().any(|value| self.contains(key, value))
    }

    fn any_key<'a, I: IntoIterator<Item = &'a str>>(&self, keys: I, value: &str) -> bool {
        keys.into_iter().any(|key| self.contains(key, value))
    }
}

fn is_station(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Tag:railway%3Dstation
    (tags.contains("railway", "station")
        && tags.any_value("station", ["subway", "light_rail", "train"]))
        || (tags.contains("railway", "station") && tags.contains("train", "yes"))
}

fn is_stop(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Tag:public%20transport=stop%20position?uselang=en
    tags.contains("railway", "stop")
        && tags.contains("public_transport", "stop_position")
        && tags.any_key(["subway", "light_rail", "train"], "yes")
}

fn is_subway(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Tag:railway%3Dsubway
    tags.any_value("railway", ["subway", "light_rail", "rail"])
}

fn is_highway(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Tag:highway%3Dmotorway
    // https://wiki.openstreetmap.org/wiki/Tag:highway%3Dtrunk
    tags.any_value(
        "highway",
        ["motorway", "trunk", "motorway_link", "trunk_link"],
    )
}

fn is_subway_route_master(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Relation:route_master
    tags.contains("type", "route_master")
        && tags.any_value("route_master", ["subway", "light_rail", "train"])
}

fn is_subway_route(tags: &Tags) -> bool {
    // https://wiki.openstreetmap.org/wiki/Tag:route%3Dsubway
    tags.contains("type", "route")
        && (tags.any_value("route", ["subway", "light_rail"])
            || tags.contains("route", "train")
                && tags.any_value("passenger", ["yes", "urban", "suburban", "local"]))
}

#[derive(Debug, Default, serde::Serialize)]
struct Output {
    subways: Vec<Way>,
    stations: Vec<Node>,
    stops: Vec<Node>,
    subway_route_masters: Vec<Relation>,
    subway_routes: Vec<Relation>,
    highways: Vec<Way>,
    keypoints: Vec<Node>,
}

impl Output {
    fn sort(&mut self) {
        self.subways.sort_by_key(|way| way.id);
        self.stations.sort_by_key(|node| node.id);
        self.stops.sort_by_key(|relation| relation.id);
        self.subway_route_masters
            .sort_by_key(|relation| relation.id);
        self.subway_routes.sort_by_key(|relation| relation.id);
        self.highways.sort_by_key(|way| way.id);
        self.keypoints.sort_by_key(|node| node.id);
    }

    fn add_node(&mut self, node: Node) {
        if is_station(&node.tags) {
            self.stations.push(node);
        } else if is_stop(&node.tags) {
            self.stops.push(node);
        }
    }

    fn add_way(&mut self, way: Way, keypoint_ids: &mut HashSet<NodeId>) {
        if is_subway(&way.tags) {
            keypoint_ids.extend(&way.nodes);
            self.subways.push(way);
        } else if is_highway(&way.tags) {
            keypoint_ids.extend(&way.nodes);
            self.highways.push(way);
        }
    }

    fn add_relation(&mut self, relation: Relation) {
        if is_subway_route_master(&relation.tags) {
            self.subway_route_masters.push(relation);
        } else if is_subway_route(&relation.tags) {
            self.subway_routes.push(relation);
        }
    }

    /// Removes an object from the output. This is very inefficient. If you want this to be faster
    /// so that you can perform bigger patches, then please make it faster.
    fn remove(&mut self, id: OsmId) {
        match id {
            OsmId::Node(id) => {
                self.stations.retain(|node| node.id != id);
                self.stops.retain(|node| node.id != id);
                // NOTE: don't need to do this to keypoints since that is populated after patching
            }
            OsmId::Way(id) => {
                self.subways.retain(|way| way.id != id);
                self.highways.retain(|way| way.id != id);
            }
            OsmId::Relation(id) => {
                self.subway_route_masters
                    .retain(|relation| relation.id != id);
                self.subway_routes.retain(|relation| relation.id != id);
            }
        }
    }
}

fn construct_tags(tags: Vec<osm_xml::Tag>) -> Tags {
    Tags(
        tags.into_iter()
            .map(|tag| (tag.key.into(), tag.val.into()))
            .collect(),
    )
}

fn decimicro(val: f64) -> i32 {
    (val * 10_i32.pow(7) as f64) as i32
}

fn apply_patch(patch: PathBuf, output: &mut Output, keypoint_ids: &mut HashSet<NodeId>) {
    let xml = osm_xml::OSM::parse(File::open(patch).unwrap()).unwrap();
    for (id, node) in xml.nodes.into_iter() {
        println!("Patching node {}", id);
        output.remove(OsmId::Node(NodeId(id)));
        output.add_node(Node {
            id: NodeId(id),
            tags: construct_tags(node.tags),
            decimicro_lat: decimicro(node.lat),
            decimicro_lon: decimicro(node.lon),
        });
    }
    for (id, way) in xml.ways.into_iter() {
        println!("Patching way {}", id);
        output.remove(OsmId::Way(WayId(id)));
        // NOTE: We will end up with orphaned keypoints. This is OK.
        output.add_way(
            Way {
                id: WayId(id),
                tags: construct_tags(way.tags),
                nodes: way
                    .nodes
                    .into_iter()
                    .map(|r| match r {
                        osm_xml::UnresolvedReference::Node(id) => NodeId(id),
                        _ => panic!("way references must be nodes"),
                    })
                    .collect(),
            },
            keypoint_ids,
        );
    }
    for (id, relation) in xml.relations.into_iter() {
        println!("Patching relation {}", id);
        output.remove(OsmId::Relation(RelationId(id)));
        output.add_relation(Relation {
            id: RelationId(id),
            tags: construct_tags(relation.tags),
            refs: relation
                .members
                .into_iter()
                .map(|m| match m {
                    osm_xml::Member::Node(osm_xml::UnresolvedReference::Node(id), role) => Ref {
                        member: OsmId::Node(NodeId(id)),
                        role: role.into(),
                    },
                    osm_xml::Member::Way(osm_xml::UnresolvedReference::Way(id), role) => Ref {
                        member: OsmId::Way(WayId(id)),
                        role: role.into(),
                    },
                    osm_xml::Member::Relation(osm_xml::UnresolvedReference::Relation(id), role) => {
                        Ref {
                            member: OsmId::Relation(RelationId(id)),
                            role: role.into(),
                        }
                    }
                    _ => panic!("unexpected unresolved reference for member {:?}", m),
                })
                .collect(),
        });
    }
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let mut pbf = OsmPbfReader::new(File::open(args.path).unwrap());

    let mut output = Output::default();
    let mut keypoint_ids: HashSet<NodeId> = HashSet::new();

    for obj in pbf.par_iter().map(Result::unwrap) {
        match obj {
            OsmObj::Node(node) => output.add_node(node),
            OsmObj::Way(way) => output.add_way(way, &mut keypoint_ids),
            OsmObj::Relation(relation) => output.add_relation(relation),
        }
    }

    // apply patches
    for patch in args.patches {
        apply_patch(patch, &mut output, &mut keypoint_ids);
    }

    pbf.rewind().unwrap();

    // pull out nodes referenced by ways
    for obj in pbf.par_iter().map(Result::unwrap) {
        if let OsmObj::Node(node) = obj {
            if keypoint_ids.contains(&node.id) {
                output.keypoints.push(node);
            }
        }
    }

    // sort for hermeticity
    output.sort();

    std::fs::write(args.output, serde_json::to_string(&output).unwrap()).unwrap();
}
