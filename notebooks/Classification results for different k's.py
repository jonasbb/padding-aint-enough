# -*- coding: utf-8 -*-
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

import matplotlib.cm
import matplotlib.pyplot as plt
import numpy as np

from common_functions import autolabel

# %%
fname = "../results/2019-01-11-closed-world/statistics-final.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-2.16.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-4.0.csv"
res: t.Dict[str, t.List[int]] = {}
csv_reader = csv.reader(open(fname))
labels = next(csv_reader)[2:-1]
for row in csv_reader:
    # The first two entries are k and label
    # The last entry is the number of distinct reasons
    # We do not want to count those values
    good_values = row[2:-1]
    key_k = row[0]
    stats = res.get(key_k, [0] * len(good_values))
    res[key_k] = [x + int(y) for x, y in zip(stats, good_values)]
del csv_reader

# %%
# Combine the values for `x` and `x_w_reason`
res = {
    key: [values[i] + values[i + 1] for i in range(0, len(values), 2)]
    for key, values in res.items()
}
labels = [labels[i] for i in range(0, len(labels), 2)]

# %%
# Check that each k-values has the same number of traces
num_traces = [sum(v) for v in res.values()]
for v in num_traces:
    assert v == num_traces[0]
total_traces = num_traces[0]

# %%
# Cut out the classification results we do not care for
# Namely: No Result, Wrong, and Contains
res = {key: values[3:] for key, values in res.items()}
# Use real proper labels
labels = ["Pseudo-Plurality + Distance", "Plurality", "Majority", "Exact"]

# %%
# Transpose res, such that for each label, we have a list of values for increasing k's
res_label: t.Dict[str, t.List[int]] = {}
res_keys = sorted(res.keys())
for l_idx, label in enumerate(labels):
    res_label[label] = []
    for res_key in res_keys:
        res_label[label].append(res[res_key][l_idx])

# %%
plt.close()
plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})
colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
hatches = cycle(["/", "-", "\\", "|"])

last_values = np.array([0] * len(res_label[labels[0]]))
for label in labels[::-1]:
    values = res_label[label]
    # Convert into percentages
    pv = [v * 100 / total_traces for v in values]
    pb = [v * 100 / total_traces for v in last_values]
    bar = plt.bar(
        range(1, 1 + len(values)),
        pv,
        label=label,
        color=next(colors),
        hatch=next(hatches),
        # Make them a tiny bit wider than they need to be in order to avoid white lines between the bars
        width=1.01,
        bottom=pb,
    )
    last_values += values
autolabel(bar, plt)

plt.legend(loc="upper center", ncol=4, mode="expand")

# CAREFUL: Those are tiny spaces around the =
plt.xticks(range(1, 1 + len(res.keys())), [f"k = {k}" for k in res.keys()])
plt.xlim(0.5, len(res_label[labels[0]]) + 0.5)
plt.ylim(0, 100)
plt.ylabel("Percent of all DNS sequences")

plt.gcf().set_size_inches(7, 4)
plt.tight_layout()
plt.savefig(f"classification-results-{path.basename(fname)}.svg")

# %%
[v * 100 / total_traces for v in last_values]

# %%

