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
# pylint: disable=redefined-outer-name,global-statement

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
# Use real proper labels
LABELS = ["Pseudo-Plurality + Distance", "Plurality", "Majority", "Exact"]

# %%
def load_stats_file(fname: str) -> t.Tuple[t.Dict[str, t.List[int]], int]:
    global LABELS
    res: t.Dict[str, t.List[int]] = {}
    csv_reader = csv.reader(open(fname))
    # Read the header
    next(csv_reader)
    for row in csv_reader:
        # The first two entries are k and label
        # The last entry is the number of distinct reasons
        # We do not want to count those values
        good_values = row[2:-1]
        key_k = row[0]
        stats = res.get(key_k, [0] * len(good_values))
        res[key_k] = [x + int(y) for x, y in zip(stats, good_values)]
    del csv_reader

    # Combine the values for `x` and `x_w_reason`
    res = {
        key: [values[i] + values[i + 1] for i in range(0, len(values), 2)]
        for key, values in res.items()
    }

    # Check that each k-values has the same number of traces
    num_traces = [sum(v) for v in res.values()]
    for v in num_traces:
        assert v == num_traces[0]
    total_traces = num_traces[0]

    # Cut out the classification results we do not care for
    # Namely: No Result, Wrong, and Contains
    res = {key: values[3:] for key, values in res.items()}

    # Transpose res, such that for each label, we have a list of values for increasing k's
    res_label: t.Dict[str, t.List[int]] = {}
    res_keys = sorted(res.keys())
    for l_idx, label in enumerate(LABELS):
        res_label[label] = []
        for res_key in res_keys:
            res_label[label].append(res[res_key][l_idx])

    return res_label, total_traces


# %%
fname = "../results/2019-01-11-closed-world/statistics-final.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-2.16.csv"
# fname = "../results/2019-01-11-closed-world/statistics-final-dist-4.0.csv"
fname_a = "../results/2019-02-04-scenario4/scenario4-stats.csv"
fname_b = "../results/2019-02-04-scenario4/scenario4-b-stats.csv"

# # Use this for a single file
# res_label, total_traces = load_stats_file(fname)
# res_label_err = None

# Use this when plotting the average of two files
res_label_a, total_traces_a = load_stats_file(fname_a)
res_label_b, total_traces_b = load_stats_file(fname_b)
res_label = {
    l: [a + b for a, b in zip(res_label_a[l], res_label_b[l])]
    for l in res_label_a.keys()
}
total_traces = total_traces_a + total_traces_b
# Calculate error bars, by suming over all labels, and then comparing these
_a = [sum(x) for x in zip(*res_label_a.values())]
_b = [sum(x) for x in zip(*res_label_b.values())]
res_label_err = [abs(a - b) / 2 for a, b in zip(_a, _b)]

# %%
plt.close()
plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})
colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
hatches = cycle(["/", "-", "\\", "|"])

last_values = np.array([0] * len(res_label[LABELS[0]]))
for label in LABELS[::-1]:
    kwargs: t.Dict[str, t.Any] = {}
    values = res_label[label]
    # Convert into percentages
    pv = [v * 100 / total_traces for v in values]
    pb = [v * 100 / total_traces for v in last_values]

    # Plot error bars, if available
    if res_label_err and "Pseudo" in label:
        kwargs["yerr"] = [v * 100 / total_traces for v in res_label_err]
        kwargs["error_kw"] = {"lw": 5}

    bar = plt.bar(
        range(1, 1 + len(values)),
        pv,
        label=label,
        color=next(colors),
        hatch=next(hatches),
        # Make them a tiny bit wider than they need to be in order to avoid white lines between the bars
        width=1.01,
        bottom=pb,
        **kwargs,
    )
    last_values += values

yoffset = None
if res_label_err:
    yoffset = [v * 100 / total_traces for v in res_label_err]
autolabel(bar, plt, yoffset=yoffset)

plt.legend(loc="upper center", ncol=4, mode="expand")

# CAREFUL: Those are tiny spaces around the =
plt.xticks(range(1, 1 + 5), [f"k = {k}" for k in range(1, 10, 2)])
plt.xlim(0.5, len(res_label[LABELS[0]]) + 0.5)
plt.ylim(0, 100)
plt.ylabel("Percent of all DNS sequences")

plt.gcf().set_size_inches(7, 4)
plt.tight_layout()
plt.savefig(f"classification-results-{path.basename(fname)}.svg")

# %%

