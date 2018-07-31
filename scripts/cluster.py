#!/usr/bin/env python3
import os
import typing as t
from glob import glob

import numpy as np
import pylib
from matplotlib import pyplot as plt
from scipy.cluster.hierarchy import dendrogram, linkage


def labels(sequences: t.List[pylib.Sequence]) -> t.List[str]:
    return [os.path.basename(seq.id()).replace(".dnstap.xz", "") for seq in sequences]


def main() -> None:
    files = []
    for domain in ["adobe.com", "amazon.*", "t.co"]:
        files.extend(
            glob(f"/mnt/data/Downloads/new-task-setup/processed.old/{domain}/*dnstap*")
        )
    seqs = [pylib.load_file(file) for file in files]

    dists = np.ndarray(shape=(len(seqs), len(seqs)), dtype=int)
    for i, s1 in enumerate(seqs):
        for j, s2 in enumerate(seqs):
            if i <= j:
                dists[i, j] = s1.distance(s2)
            else:
                dists[i, j] = -1

    Z = linkage(dists, method="ward", optimal_ordering=True)
    _fig = plt.figure(figsize=(25, 10))
    _dn = dendrogram(
        Z,
        labels=labels(seqs),
        orientation="left",
        show_leaf_counts=True,
        show_contracted=True,
        color_threshold=6000,
    )
    plt.show()


if __name__ == "__main__":
    main()
