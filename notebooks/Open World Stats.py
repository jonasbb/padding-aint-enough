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
# # %matplotlib notebook
# %matplotlib inline

import json
import lzma
import typing as t
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

import matplotlib.pyplot as plt
import pylib

from common_functions import show_infos_for_domain

# %%
@dataclass
class SequenceStats:
    seq_length: int
    min_dist: int
    min_dist_label: str
    max_dist: int
    max_dist_label: str

# %%
fname = "../results/2019-01-15-open-world-no-thres/misclassifications-open-world.json.xz"
content = []
with lzma.open(fname, 'rb') as f:
    lines = iter(f)
    # skip header
    next(lines)
    for line in lines:
        try:
            content.append(json.loads(line))
        except Exception:
            # Catch parse errors
            pass

# %%
content[-1]

# %% [markdown]
# # Most common misclassifications

# %%
most_common_misclassification: t.Counter[str] = Counter()
for entry in content:
    if entry["reason"] is not None:
        continue
    for mis in entry["class_result"]["options"]:
        if int(mis["distance_min"]) < 100000:
            most_common_misclassification.update([mis["name"]] * mis["count"])

# %%
most_common_misclassification.most_common(20)

# %%
show_infos_for_domain("qichacha.com")

# %% [markdown]
# # Minimal / Maximal distance within block of misclassifications

# %%
# Load all local dnstap files with the current pylib version
# The current pylib version might be newer and have more classifications
local_root = "/mnt/data/Downloads/dnscaptures-open-world/"
# index by filename
cache = {}
for _, seqs in pylib.load_folder(local_root):
    for seq in seqs:
        cache[Path(seq.id()).name] = seq

local_root = "/mnt/data/Downloads/dnscaptures-main-group/"
for _, seqs in pylib.load_folder(local_root):
    for seq in seqs:
        cache[Path(seq.id()).name] = seq

# %%
# for each k, count min and max distances, as well as sequence lengths
# Map "k" -> "file name" -> Stats
distances_per_k: t.Dict[int, t.Dict[str,  SequenceStats]] = {}
for entry in content:
    # These two domains have broken sequences (SERVFAIL in CF responses)
    # and their distances are wrong
    if entry['label'] in ['szedu.net', 'tedu.cn']:
        continue

    d = distances_per_k.setdefault(entry["k"], {})

    # current sequences length
    fname = Path(entry["id"]).name
    curr_length: int = cache[fname].len()

    # Get min and max distance
    min_dist = 999999
    min_dist_label = ""
    max_dist = 0
    max_dist_label = ""
    for mis in entry["class_result"]["options"]:
        if int(mis["distance_min"]) < min_dist:
            min_dist = int(mis["distance_min"])
            min_dist_label = mis["name"]
        if int(mis["distance_max"]) > max_dist:
            max_dist = int(mis["distance_max"])
            max_dist_label = mis["name"]

    stats = SequenceStats(curr_length, min_dist, min_dist_label, max_dist, max_dist_label)
    d[fname] = stats

# %%
min_dists_norm = {
    key: sorted(
        [float(stat.min_dist)/stat.seq_length for stat in stats.values()]
    )
    for key, stats in distances_per_k.items()
}
max_dists_norm = {
    key: sorted(
        [float(stat.max_dist)/stat.seq_length for stat in stats.values()]
    )
    for key, stats in distances_per_k.items()
}

# %%
plt.plot(range(len(min_dists_norm[1])), min_dists_norm[1], label = "Min")
for key, values in list(max_dists_norm.items())[1:]:
    plt.plot(range(len(values)), values, label = f"Max k={key}")
# plt.ylim(0, 4)
plt.hlines(y=2.16, xmin=0, xmax=200000, color='black')
plt.gcf().set_size_inches(15,10)
plt.legend()

# %%
# Count how many traces will not match due to the distance threshold
for k, values in max_dists_norm.items():
    print(k, sum([1 for v in values if v <= 2.16]))

# %%
# Values which are too extreme
for fname, stat in distances_per_k[1].items():
    if float(stat.min_dist)/stat.seq_length >= 6:
        print(fname, stat)

# %%

