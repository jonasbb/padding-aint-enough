#!/usr/bin/env python3
import os
import sys
import typing as t
from glob import glob

import pylib
import scipy.cluster.hierarchy as cluster
from matplotlib import pyplot as plt
from natsort import natsorted
from scipy.spatial.distance import pdist


def labels(sequences: t.List[pylib.Sequence]) -> t.List[str]:
    return [os.path.basename(seq.id()).replace(".dnstap.xz", "") for seq in sequences]


def help(pgrm: str) -> None:
    print(
        f"""Usage: ./{pgrm} PATTERN [PATTERN [...]]

    Each PATTERN is either the path to a file loadable with `pylib.load_file`
    or a glob pattern for a set of files."""
    )
    sys.exit(1)


def main() -> None:
    if len(sys.argv) < 2:
        help(sys.argv[0])

    files: t.List[str] = []
    for arg in sys.argv[1:]:
        files += glob(arg)
    sequences = natsorted(
        (pylib.load_file(file) for file in files), key=lambda seq: seq.id()
    )

    # pdist requires a 2-dimensional array, without any good reason
    # So convert the list into an list of 1-element lists to fullfill this requirement
    # The comparison lambda just has to take the 0th element every time
    sequences_matrix = [[s] for s in sequences]
    distances_pairwise = pdist(
        sequences_matrix, lambda a, b: a[0].distance(b[0]) / max(a[0].len(), b[0].len())
    )

    for (threshold, method) in [
        (2000, "single"),
        # (3000, "average"),
        # (3000, "weighted"),
        # (3000, "centroid"),
        # (3000, "median"),
        (6000, "ward"),
    ]:
        Z = cluster.linkage(distances_pairwise, method=method, optimal_ordering=True)
        _fig = plt.figure(figsize=(15, len(files) * 0.15))
        _dn = cluster.dendrogram(
            Z,
            color_threshold=threshold,
            distance_sort="ascending",
            labels=labels(sequences),
            orientation="right",
            show_contracted=True,
            show_leaf_counts=True,
        )
        plt.savefig(f"cluster-{method}.svg", bbox_inches="tight")


if __name__ == "__main__":
    main()
