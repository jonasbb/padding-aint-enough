# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 0.8.6
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
# %matplotlib inline
# # %matplotlib notebook

import csv
import typing as t
from itertools import cycle
from os import path

import matplotlib.pyplot as plt
import numpy as np

# %%
fname = "../results/2019-01-11-closed-world/statistics-final.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-2.16.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-4.0.csv"
res: t.Dict[str, t.List[int]] = {}
csv_reader = csv.reader(open(fname))
labels = next(csv_reader)[2:-1]
# Remove the no result columns, as they are always 0
labels = labels[2:]
for row in csv_reader:
    # The first two entries are k and label
    # The last entry is the number of distinct reasons
    # We do not want to count those values
    good_values = row[2:-1]
    key_k = row[0]
    stats = res.get(key_k, [0] * len(good_values))
    res[key_k] = [x + int(y) for x, y in zip(stats, good_values)]
del csv_reader
# res

# %%
# calculate inclusive numbers, meaning this result OR better
# Like: plurality_and_dist = plurality_and_dist + plurality + majority + exact
res_cum_sum = {}
for key, values in res.items():
    good_values = values[-8:]
    good_values = list(np.cumsum(good_values[::-1]))[::-1]
    res_cum_sum[key] = values[:-8] + good_values
# res_cum_sum

# %%
# Transpose res, such that for each label, we have a list of values for increasing k's
# res_to_use = res
res_to_use = res_cum_sum
res_label: t.Dict[str, t.List[int]] = {}
res_keys = sorted(res_to_use.keys())
for l_idx, label in enumerate(labels):
    res_label[label] = []
    for res_key in res_keys:
        res_label[label].append(res_to_use[res_key][l_idx+2])

# %%
plt.close()
colors = cycle(
    [
        "#2ca02c",
        "#98df8a",
        "#bcbd22",
        "#dbdb8d",
        "#1f77b4",
        "#aec7e8",
        "#9467bd",
        "#c5b0d5",
        "#ff7f0e",
        "#ffbb78",
        "#d62728",
        "#ff9896",
        "#333333",
        "#000000",
    ]
)
markers = cycle(["+", "."])
for label in labels[::-1]:
    values = res_label[label]
    c = next(colors)
    m = next(markers)
    plt.plot(range(len(values)), values, label=label, color=c, marker=m)
plt.legend(loc="upper left", bbox_to_anchor=(1, 1))
plt.xticks(range(len(res_label[labels[0]])), ["k=1", "k=3", "k=5", "k=7", "k=9"])
plt.gcf().set_size_inches(10,5)
plt.tight_layout()
plt.savefig(f"classification-results-{path.basename(fname)}.png")
