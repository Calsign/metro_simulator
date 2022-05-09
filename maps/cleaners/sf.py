def filter_subway_route(subway_route):
    # this one is incorrectly tagged
    if "Altamont Corridor Express" in subway_route.tags["name"]:
        return False

    return True


def metros(osm):
    osm.subway_routes[:] = [
        subway_route
        for subway_route in osm.subway_routes
        if filter_subway_route(subway_route)
    ]
