# -*- coding: utf-8 -*-
# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 1.1.2
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
# %matplotlib inline

import json
import re
import typing as t

import matplotlib.pyplot as plt
import numpy as np
from common_functions import parse_log_data
from matplotlib.colors import hsv_to_rgb, rgb_to_hsv

# %%
plt.rcParams["figure.figsize"] = [15, 10]
plt.rcParams["figure.figsize"] = [7.5, 5]


# %%
def atoi(text):
    return int(text) if text.isdigit() else text


def natural_keys(text):
    """
    alist.sort(key=natural_keys) sorts in human order
    http://nedbatchelder.com/blog/200712/human_sorting.html
    (See Toothy's implementation in the comments)
    """
    return [atoi(c) for c in re.split(r"([\d.]+)", text)]


# %%
overheads = json.load(open("overheads-merged.json"))

next(iter(overheads.values()))


# %%
def name2color(name: str) -> t.List[float]:
    if name.startswith("ap-"):
        return rgb_to_hsv([0, 0, 1])
    elif name.startswith("cr-12"):
        return rgb_to_hsv([1, 0, 0])
    elif name.startswith("cr-25"):
        return rgb_to_hsv([0, 1, 0])
    elif name.startswith("cr-50"):
        return rgb_to_hsv([1, 0, 1])
    elif name.startswith("cr-100"):
        return rgb_to_hsv([0, 1, 1])
    else:
        return rgb_to_hsv([0, 1, 0])


def adjust_hue_grey(name: str, hsv: t.List[float]) -> t.List[float]:
    """Takes a HSV value, returns an RGB"""
    prob = re.findall(r"([\d.]+p)", name)[0]
    # remove the trailing p
    prob = float(prob[:-1]) / 2 + 0.5 - 0.3
    hsv[-1] = prob
    return tuple(hsv_to_rgb(hsv))


def adjust_hue(name: str, hsv: t.List[float]) -> t.List[float]:
    """Takes a HSV value, returns an RGB"""
    prob = re.findall(r"([\d.]+p)", name)[0]
    # remove the trailing p
    prob = float(prob[:-1])
    hsv[-1] = prob
    return tuple(hsv_to_rgb(hsv))


# %%

# %%
a = [0] + np.sin(np.linspace(0, 2 * np.pi, 50)).tolist()
b = [0] + np.cos(np.linspace(0, 2 * np.pi, 50)).tolist()
full_circle = np.column_stack([a, b])
circle_size = 200

# grey in HSV
# Hue, Saturation, Value (lightness)
grey = [0, 0, 0.5]

legends = []
name2name = {
    "ap-2length": "Adaptive Padding",
    "cr-12ms": "CR 12 ms",
    "cr-25ms": "CR 25 ms",
    "cr-50ms": "CR 50 ms",
    "cr-100ms": "CR 100 ms",
}

fig, axes = plt.subplots(1, 3, sharey=True, gridspec_kw={"width_ratios": [3, 1, 1]})

# for x,y in [(x,y) for x in range(7) for y in range(7)]:
for name, overhead in sorted(overheads.items(), key=lambda x: natural_keys(x[0])):
    if "5p" in name and ".5p" not in name:
        continue
    if "0.1p" in name or "0.2p" in name or "0.3p" in name:
        continue
    #     if "cr" in name:
    #         continue
    if "ap-" in name or "cr-12ms" in name or "cr-25ms" in name:
        ax = axes[0]
    elif "cr-50ms" in name:
        ax = axes[1]
    elif "cr-100ms" in name:
        ax = axes[2]

    x = float(overhead["time"]) / float(overhead["time_baseline"]) + 1
    y = float(overhead["queries"]) / float(overhead["queries_baseline"]) + 1

    res = None
    for ext in [".json.xz.log", ".json.xz.cr.log"]:
        try:
            res = dict(parse_log_data(f"/home/jbushart/results/{name}{ext}")[0])[
                "PluralityThenMinDist"
            ]
            break
        except Exception:
            pass
    else:
        #         raise OSError(2, "No such file or directory", name)
        res = [100] + ([1] * 10)
    print(name, res)
    print(
        "   ",
        "5/10 (%):",
        res[5] / res[0] * 100,
        "Query Overhead (%):",
        (y - 1) * 100,
        "Time Overhead (%):",
        (x - 1) * 100,
    )
    # X / 10 correct
    frac = res[5] / res[0] * 10

    #     frac = 1/(x+y) if x+y>0 else 1
    a = [0] + np.sin(np.linspace(0, 2 * np.pi * frac, 50)).tolist()
    b = [0] + np.cos(np.linspace(0, 2 * np.pi * frac, 50)).tolist()
    ab = np.column_stack([a, b])
    s = np.abs(ab).max()
    ax.scatter(
        [x],
        [y],
        marker=full_circle,
        s=s * circle_size,
        facecolor=adjust_hue_grey(name, grey),
        alpha=0.2,
        zorder=10,
        linewidths=0,
    )
    color = adjust_hue(name, name2color(name))
    ax.scatter(
        [x],
        [y],
        marker=ab,
        s=s * circle_size,
        facecolor=color,
        label=name,
        alpha=0.5,
        zorder=100,
        linewidths=0,
    )

    if "-0.9p" in name:
        name = name[: -len("-0.9p")]
        handle = ax.scatter(
            [-1],
            [-1],
            marker=full_circle,
            s=s * 200,
            facecolor=color,
            label=name,
            linewidths=0,
        )
        legends.append((handle, name2name[name]))

for ax in axes:
    ax.set_ylim(bottom=1, top=3.5)
axes[0].set_xlim(left=1 - 0.0025, right=1.01)
# axes[0].set_xticks([0.995, 1, ])
axes[1].set_xlim(left=1.02, right=1.04)
axes[2].set_xlim(left=1.18, right=1.2)
# plt.xlim(left=0.995, right=1.005)

# Draw a tiny arrow describing the probability values
axes[0].arrow(
    1.00625,
    2.7,
    0,
    1.3 - 2.7,
    length_includes_head=True,
    color="black",
    width=0.00001,
    head_width=0.0005,
    head_length=0.1,
)
axes[0].text(1.00675, 2.6, "p = 0.9")
axes[0].text(1.00675, 1.42, "p = 0.4")

# Solution copied from: https://stackoverflow.com/a/53172335
# add a big axis, hide frame
fig.add_subplot(111, frameon=False)
# hide tick and tick label of the big axis
plt.tick_params(labelcolor="none", top=False, bottom=False, left=False, right=False)
plt.xlabel("Time Overhead (relative to baseline)")
plt.ylabel("Query Overhead (relative to baseline)")

plt.gcf().set_size_inches(7, 6)
# plt.gcf().set_size_inches(3, 4)
plt.tight_layout()
plt.savefig(f"mitigations-overhead-tradeoff.svg")
plt.show()

# Create legend
fig = plt.figure()
ax = fig.add_subplot(111)
ax.set_axis_off()
fig.legend(
    [x[0] for x in legends],
    [x[1] for x in legends],
    ncol=5,
    loc="center",
    frameon=False,
)
fig.tight_layout()
fig.savefig(f"mitigations-overhead-tradeoff-legend.svg")
fig.show()

# %%
