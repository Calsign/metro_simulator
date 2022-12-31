import typing as T

import math
from dataclasses import dataclass
from functools import lru_cache


# Kilometers per degree at the equator
EQ_KM_PER_DEG = 111


@dataclass
class Coords:
    lat: float
    lon: float
    radius: float  # meters

    @property
    def lon_radius(self):
        # account for curvature of the earth
        return self.radius / 1000 / EQ_KM_PER_DEG / math.cos(math.radians(self.lat))

    @property
    def lat_radius(self):
        return self.radius / 1000 / EQ_KM_PER_DEG


@dataclass
class MapConfig:
    name: str

    latitude: str
    longitude: str

    engine_config: dict
    datasets: T.Dict[str, T.Any]


def round_to_pow2(x, up=True):
    """
    Round up or down to the nearest power of two.
    """
    f = (math.floor, math.ceil)[up]
    return int(2 ** f(math.log(x, 2)))


def centered_box(lon, lat, lon_radius, lat_radius, transform):
    assert -180 <= lon < 180, lon
    assert -90 <= lat <= 90, lat

    lon_px = math.floor((lon - transform.lon_min) / transform.lon_res)
    lat_px = math.floor((lat - transform.lat_min) / transform.lat_res)
    lon_rad = int(lon_radius / abs(transform.lon_res))
    lat_rad = int(lat_radius / abs(transform.lat_res))
    return ((lon_px - lon_rad, lat_px - lat_rad), (lon_px + lon_rad, lat_px + lat_rad))


def round_coords(point) -> T.Tuple[float, float]:
    """
    Allows us to use a (float, float) pair as a key in a dictionary.
    Basically, rounding floats before comparing them allows for small
    discrepancies to be ignored.
    """
    if hasattr(point, "x") and hasattr(point, "y"):
        (x, y) = (point.x, point.y)
    else:
        (x, y) = point

    # round to 2 decimal places
    # TODO: there's some correctness issue here that I don't fully understand
    return (round(x, 2), round(y, 2))


@lru_cache
def address_from_coords(x: int, y: int, max_depth: int) -> T.List[int]:
    max_dim = 2**max_depth

    min_x = 0
    max_x = max_dim
    min_y = 0
    max_y = max_dim

    quadrant_map = {
        (False, False): 0,
        (True, False): 1,
        (False, True): 2,
        (True, True): 3,
    }

    address = []

    for _ in range(max_depth):
        cx = (max_x + min_x) / 2
        cy = (max_y + min_y) / 2
        right = x >= cx
        bottom = y >= cy

        if right:
            min_x = cx
        else:
            max_x = cx

        if bottom:
            min_y = cy
        else:
            max_y = cy

        address.append(quadrant_map[(right, bottom)])

    return address
