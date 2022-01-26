import math
from dataclasses import dataclass

import typing as T


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
        return self.radius / 1000 / EQ_KM_PER_DEG / \
            math.cos(math.radians(self.lat))

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
