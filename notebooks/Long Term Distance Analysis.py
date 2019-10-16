# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 1.2.4
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
# pylint: disable=redefined-outer-name

# %%
import typing as t
from glob import glob

import matplotlib.pyplot as plt
import numpy as np
import pylib
import tldextract
from natsort import natsorted

# %%
# Configure tldextract
extract = tldextract.TLDExtract(include_psl_private_domains=False)
extract.update()

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [15, 10]


# %%
def cross_distances(
    list_a: t.List[pylib.Sequence], list_b: t.List[pylib.Sequence]
) -> t.List[int]:
    res = list()
    for a in list_a:
        for b in list_b:
            res.append(a.distance(b))
    return res


# %%
def calculate_total(values: t.ValuesView[t.List[t.List[int]]]) -> t.List[t.List[int]]:
    return list(map(lambda x: [sum(x)], zip(*map(lambda x: map(sum, x), values))))


# %%
def smooth(y: t.Sequence[float], box_pts: int) -> t.List[float]:
    y_windowed = (y[i : i + box_pts] for i in range(len(y)))

    def smoother(vals: t.Sequence[float]) -> float:
        # ignore the highest outlier(s)
        vals = sorted(vals)
        vals = vals[:-2]
        if len(vals) == 0:
            vals.append(0)
        return sum(vals) / len(vals)

    return list(map(smoother, y_windowed))


# %% [markdown]
# # Dataset
#
# * 20 domain
# * 5 samples per domain
# * 682 iterations
# * Duration: ~64.5 hours

# %%
source = "../../data/long-term-data/*/"
source = "/mnt/data/Downloads/dnscaptures-2019-10-07-7days/*/"

folders = natsorted(glob(source))

# %%
data = [
    {domain: sequences for domain, sequences in pylib.load_folder(f, "json")}
    for f in folders
]

# %%
distances: t.Dict[str, t.List[t.List[int]]] = dict()
for domain in data[0].keys():
    for run in data[1:]:
        distances.setdefault(domain, list()).append(
            cross_distances(data[0][domain], run.get(domain, list()))
        )

distances_relative: t.Dict[str, t.List[t.List[int]]] = dict()
for domain in data[0].keys():
    for run0, run1 in zip(data[0:], data[1:]):
        distances_relative.setdefault(domain, list()).append(
            cross_distances(run0.get(domain, list()), run1.get(domain, list()))
        )

# %%
alpha_original = 0.4
alpha_smoothed = 1.0
total = "total.total"


def plot(values: t.Sequence[float], label: str, if_: bool) -> None:
    c = next(colors)
    if if_:
        values_smooth = smooth(values, 10)
        plt.plot(values, label=label, color=c, alpha=alpha_original)
        plt.plot(values_smooth, color=c, alpha=alpha_smoothed)


for extra, dst in [("", distances), ("relative-", distances_relative)]:
    dst[total] = calculate_total(dst.values())
    for domain, values in dst.items():
        colors = iter([f"C{i}" for i in range(10)])

        plt.close()
        plt.title(domain)
        ys_max = [max(step, default=0) for step in values]
        ys_min = [min(step, default=0) for step in values]
        ys_mean = [sum(step) / len(step) if len(step) > 0 else 0 for step in values]
        ys_median = [np.median(step) if len(step) > 0 else 0 for step in values]

        plot(ys_max, "Max Distance", domain != total)
        plot(ys_mean, "Mean Distance", False)
        plot(ys_median, "Median Distance", True)
        plot(ys_min, "Min Distance", domain != total)

        plt.ylabel("Absolute Distance")
        plt.xlabel("Iteration")
        plt.ylim(bottom=0, top=sorted(ys_max)[int(len(ys_max) * 0.95)] * 1.5)
        plt.xlim(left=0, right=len(values))
        plt.legend()
        #     plt.show()
        plt.savefig(f"domain-{extra}{domain}.png")

# %%
md = []
md.append("|Domain|Distances compared to start|Distances compared to previous run|")
md.append("|--:|:-:|:-:|")
for domain in distances.keys():
    res = extract(domain)
    formatted_domain = ""
    if res[0] != "":
        formatted_domain += res[0] + "."
    formatted_domain += f"**{res[1]}**.{res[2]}"

    md.append(
        f"""|{formatted_domain}|![](domain-{domain}.png)|![](domain-relative-{domain}.png)|"""
    )
with open("overview.md", "w+t") as f:
    f.write("\n".join(md))

# %%
