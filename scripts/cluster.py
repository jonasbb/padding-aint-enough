#!/usr/bin/env python3
import os
import sys
import typing as t
from glob import glob

import pylib
from matplotlib import pyplot as plt
from scipy.cluster.hierarchy import dendrogram, linkage


def labels(sequences: t.List[pylib.Sequence]) -> t.List[str]:
    return [os.path.basename(seq.id()).replace(".dnstap.xz", "") for seq in sequences]


def main() -> None:
    files = glob(os.path.join(sys.argv[1], "*", "*dnstap*"))
    seqs = sorted((pylib.load_file(file) for file in files), key=lambda seq: seq.id())

    dists = []
    for i, s1 in enumerate(seqs):
        for j, s2 in enumerate(seqs):
            if i < j:
                dists.append(s1.distance(s2))

    for (threshold, method) in [
        (2000, "single"),
        # (3000, "average"),
        # (3000, "weighted"),
        # (3000, "centroid"),
        # (3000, "median"),
        (6000, "ward"),
    ]:
        Z = linkage(dists, method=method, optimal_ordering=True)
        _fig = plt.figure(figsize=(15, len(files) * 0.15))
        _dn = dendrogram(
            Z,
            labels=labels(seqs),
            orientation="left",
            show_leaf_counts=True,
            show_contracted=True,
            color_threshold=threshold,
        )
        plt.savefig(f"cluster-{method}.svg", bbox_inches="tight")


if __name__ == "__main__":
    main()
