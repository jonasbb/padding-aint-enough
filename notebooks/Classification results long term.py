# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.3'
#       jupytext_version: 1.3.0rc0+dev
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
from glob import glob
from itertools import cycle

import matplotlib.cm
import matplotlib.pyplot as plt
import numpy as np
from natsort import natsorted


# %%
def load_statistics_csv(fname: str) -> t.Tuple[int, t.Dict[str, t.List[int]]]:
    # pylint: disable=W0621
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

    # Combine the values for `x` and `x_w_reason`
    res = {
        key: [values[i] + values[i + 1] for i in range(0, len(values), 2)]
        for key, values in res.items()
    }
    labels = [labels[i] for i in range(0, len(labels), 2)]

    # Check that each k-values has the same number of traces
    num_traces = [sum(v) for v in res.values()]
    for v in num_traces:
        assert v == num_traces[0]
    total_traces = num_traces[0]

    # Cut out the classification results we do not care for
    # Namely: No Result, Wrong, and Contains
    res = {key: values[3:] for key, values in res.items()}
    # Use real proper labels
    labels = ["Pseudo-Plurality + Distance", "Plurality", "Majority", "Exact"]

    # Transpose res, such that for each label, we have a list of values for increasing k's
    res_label: t.Dict[str, t.List[int]] = {}
    res_keys = sorted(res.keys())
    for l_idx, label in enumerate(labels):
        res_label[label] = []
        for res_key in res_keys:
            res_label[label].append(res[res_key][l_idx])

    return (total_traces, res_label)


# %%
# Use real proper labels
labels = ["Pseudo-Plurality + Distance", "Plurality", "Majority", "Exact"]

# %%
# Load the statistics data for all iterations of the long term
# basepath = "../results/2019-01-24-long-term/statistics-*.csv"
basepath = "../tmpres/stats-*.csv"
data = [load_statistics_csv(f) for f in natsorted(glob(basepath))]

# %%
pdata: t.Dict[str, t.List[int]] = {}
# create keys in pdata
pdata["total"] = []
for k in data[0][1].keys():
    pdata[k] = []

# This allows to specify which k-value we plot
# Normally this is either 0 and we ran the classification with --exact-k
# or the classification used -k, then 0 means k=1, 1 means k=3, etc.
idx = 0
for a, b in data:
    pdata["total"].append(a)
    for k, values in b.items():
        pdata[k].append(values[idx])

# %%
plt.close()
plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})
colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
hatches = cycle(["/", "-", "\\", "|"])

last_values = np.array([0] * len(pdata[labels[0]]))
for label in labels[::-1]:
    values = pdata[label]
    total_traces = pdata["total"]
    # Convert into percentages
    pb = [v * 100 / total for v, total in zip(last_values, total_traces)]
    last_values += values
    pv = [v * 100 / total for v, total in zip(last_values, total_traces)]
    x = list(range(0, len(values)))
    c = next(colors)
    # Cannot use a bar plot here, as there are too many bars and they interfere and make the image too large
    # Use fill_between as alternative
    plt.fill_between(
        x,
        pv,
        y2=pb,
        step="pre",
        hatch=next(hatches),
        facecolor=c,
        label=label,
        linewidth=0,
        edgecolor="black",
    )


plt.legend(loc="upper center", ncol=4, mode="expand")

# xticks = list(range(0, len(pdata["total"]), 48))
# xticks_labels = [f"{d // 24}" for d in xticks]

# Measurements are once every half hour. So 48 entries per day
xticks = list(range(0, len(pdata["total"]), 48))
xticks_labels = [f"{d // 48}" for d in xticks]
plt.xticks(xticks, xticks_labels)
plt.xlabel("Days since start")
plt.xlim(0, len(pdata["total"]))

plt.ylim(0, 100)
plt.ylabel("Percent of all DNS sequences")

plt.gcf().set_size_inches(7, 4)
plt.tight_layout()
plt.savefig(f"classification-results-long-term.svg")

# %%
