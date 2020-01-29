# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.3'
#       jupytext_version: 1.3.0
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
# pylint: disable=redefined-outer-name

# %matplotlib inline

import csv
import typing as t
from copy import deepcopy

import matplotlib.pyplot as plt
import numpy as np
from common_functions import COLORS2, HATCHES, LABELS, open_file


# %%
def load_stats_file(fname: str) -> t.Tuple[t.Dict[str, t.List[int]], int]:
    res: t.Dict[str, t.List[int]] = {}
    csv_reader = csv.reader(open_file(fname))
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
res = {}
for label, fname, zorder in [
    # ("Baseline", "../results/2019-02-10-simulate/statistics-simulate-Normal.csv", 0),
    (
        "Baseline",
        "../results/2019-11-18-full-rescan/crossvalidate/crossvalidate-stats-0.csv.xz",
        0,
    ),
    (
        "Perfect Timing",
        # "../results/2019-02-10-simulate/statistics-simulate-PerfectTiming.csv",
        "../results/2019-11-18-full-rescan/countermeasures/stats-stats-perfect-timing.csv.xz",
        100,
    ),
    (
        "Perfect Padding",
        # "../results/2019-02-10-simulate/statistics-simulate-PerfectPadding.csv",
        "../results/2019-11-18-full-rescan/countermeasures/stats-stats-perfect-padding.csv.xz",
        50,
    ),
]:
    res_label, total_traces = load_stats_file(fname)
    # sum up all the counts in res_label
    tmp = [sum(v) for v in zip(*res_label.values())]
    res[label] = (tmp, total_traces, zorder)


# %%
plt.close()
plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})
colors = deepcopy(COLORS2)
hatches = deepcopy(HATCHES)

last_values = np.array([0] * (len(next(iter(res.values()))[0]) + 1))
legend = []
for label, (values, total_traces, zorder) in res.items():
    kwargs: t.Dict[str, t.Any] = {"step": "post", "zorder": zorder}
    # Convert into percentages
    values = [*values, values[-1]]
    pv = [v * 100 / total_traces for v in values]

    ln = plt.fill_between(
        range(len(values)),
        pv,
        label=label,
        facecolor=next(colors),
        hatch=next(hatches),
        **kwargs,
    )
    legend.append(ln)
    last_values += values

labs = [l.get_label() for l in legend]
plt.legend(legend, labs, loc="upper center", ncol=3, mode="expand")

# CAREFUL: Those are tiny spaces around the =
plt.xticks(
    [x + 0.5 for x in range(len(last_values))], [f"k = {k}" for k in range(1, 10, 2)]
)
plt.xlim(0, len(last_values) - 1)
plt.ylim(0, 100)
plt.ylabel("Correctly classified websites in %")

plt.gcf().set_size_inches(7, 4)
plt.tight_layout()
plt.savefig("countermeasures-evaluation.svg")

# %%
