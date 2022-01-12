
from dataclasses import dataclass


@dataclass
class ConvolveData:
    x: int
    y: int
    depth: int
    address: list[int]


class Quadtree:
    CHILD_QUADRANTS = [(0, 0), (0, 1), (1, 0), (1, 1)]

    def __init__(self, max_depth=0, data=None):
        self.max_depth = max_depth
        self.data = data
        self.children = []

    def fill(self,  data):
        if self.max_depth > 0:
            self.children = []
            for _ in range(len(Quadtree.CHILD_QUADRANTS)):
                child = Quadtree(max_depth=self.max_depth-1)
                child.fill(data)
                self.children.append(child)
        else:
            self.data = data

    def _convolve_internal(self, f, post, x, y, depth, address):
        assert len(self.children) in [0, 4]

        data = ConvolveData(x=x, y=y, depth=depth, address=address)

        if not post:
            f(self, data)
        for (i, (child, (cx, cy))) in enumerate(zip(self.children, Quadtree.CHILD_QUADRANTS)):
            child._convolve_internal(f, post, x+cx*2**(self.max_depth-1),
                                     y+cy*2**(self.max_depth-1), depth+1, address + [i])
        if post:
            f(self, data)

    def convolve(self, f, post=False):
        self._convolve_internal(
            f=f, post=post, x=0, y=0, depth=0, address=[])

    def __str__(self):
        assert len(self.children) in [0, 4]
        if len(self.children) == 0:
            return "Quadtree({})".format(self.data)
        else:
            return "Quadtree([{}, {}])".format(self.data, ", ".join([str(c) for c in self.children]))
