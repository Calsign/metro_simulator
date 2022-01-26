import math
from dataclasses import dataclass
import functools

import typing as T

import numpy as np

from generate.data import Coords, round_to_pow2, centered_box, EQ_KM_PER_DEG


@dataclass
class GeoTransform:
    lon_min: float
    lon_res: float
    lat_min: float
    lat_res: float

    @staticmethod
    def from_gdal(dataset):
        (lon_min, lon_res, _, lat_min, _, lat_res) = dataset.GetGeoTransform()
        return GeoTransform(lon_min, lon_res, lat_min, lat_res)


@functools.lru_cache
def osgeo_gdal():
    import osgeo.gdal

    osgeo.gdal.UseExceptions()
    return osgeo.gdal


def read_gdal(
    dataset: T.Dict[str, T.Any], coords: Coords, max_dim: int, band_num: int = 1
) -> np.ndarray:
    """
    Read data from a region of a (potentially tiled) dataset into a numpy array.

    Tiles must have the same resolution and cover the entire requested region.
    If tiles overlap, this function will not fail but the behavior is unspecified.

    :param dataset: a dataset; a dict with keys "tiles" (a list of paths to geotiff files)
                    and "data" (a dict with extra dataset metadata).
    :param coords: the coordinates of the region to load
    :param max_dim: the maximum width/height of the output array
    :param band_num: the GDAL band number to select
    """

    output = None
    lat_lon_res = None
    downsampled_dim = None

    total_area = 0

    # NOTE: sorted shouldn't be necessary, but for debugging it can be
    # useful for the results to be deterministic
    for data_file in sorted(dataset["tiles"]):
        data = osgeo_gdal().Open(data_file, osgeo_gdal().GA_ReadOnly)
        band = data.GetRasterBand(band_num)
        transform = GeoTransform.from_gdal(data)

        ((x1, y1), (x2, y2)) = centered_box(
            coords.lon, coords.lat, coords.lon_radius, coords.lat_radius, transform
        )

        current_lat_lon_res = (transform.lat_res, transform.lon_res)
        if output is None:
            # instantiate these values on the first pass because we need the resolution
            # this lets us load each file only once

            lat_lon_res = current_lat_lon_res

            downsample = dataset["data"]["downsample"]
            assert downsample >= 0
            downsampled_dim = min(round_to_pow2(y2 - y1), max_dim) // (2 ** downsample)

            output = np.zeros([downsampled_dim, downsampled_dim])
        else:
            assert (
                lat_lon_res == current_lat_lon_res
            ), "Got tiles with incompatible resolutions: {} != {}".format(
                lat_lon_res, current_lat_lon_res
            )

        # crop to portion in this tile
        (x1c, y1c) = (min(max(x1, 0), band.XSize), min(max(y1, 0), band.YSize))
        (x2c, y2c) = (min(max(x2, 0), band.XSize), min(max(y2, 0), band.YSize))

        if x2c - x1c == 0 or y2c - y1c == 0:
            print("Unused dataset tile: {}".format(data_file))
        else:
            print("Using dataset tile: {}".format(data_file))

            # project portion of output covered by this tile into the output space
            (dx1, dy1) = (
                round((x1c - x1) / (x2 - x1) * downsampled_dim),
                round((y1c - y1) / (y2 - y1) * downsampled_dim),
            )
            (dx2, dy2) = (
                round((x2c - x1) / (x2 - x1) * downsampled_dim),
                round((y2c - y1) / (y2 - y1) * downsampled_dim),
            )

            # let gdal take care of resampling for us
            arr = band.ReadAsArray(
                xoff=x1c,
                yoff=y1c,
                win_xsize=x2c - x1c,
                win_ysize=y2c - y1c,
                buf_xsize=dx2 - dx1,
                buf_ysize=dy2 - dy1,
            )
            output[dy1:dy2, dx1:dx2] = arr

            total_area += (dx2 - dx1) * (dy2 - dy1)

        # not necessary, but make clear that we no longer need this tile and it should be closed
        del data

    assert downsampled_dim is not None
    assert output is not None
    assert (
        total_area >= downsampled_dim ** 2
    ), "Missing tiles, areas unequal: {} < {}".format(total_area, downsampled_dim ** 2)

    return output
