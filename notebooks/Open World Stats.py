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
# pylint: disable=broad-except
# # %matplotlib notebook
# %matplotlib inline

import json
import lzma
import typing as t
from dataclasses import dataclass
from pathlib import Path

import matplotlib.pyplot as plt
import pylib


# %%
@dataclass
class SequenceStats:
    seq_length: int
    min_dist: float
    min_dist_label: str
    max_dist: float
    max_dist_label: str


# %%
fname = (
    "../results/2019-01-15-open-world-no-thres/misclassifications-open-world.json.xz"
)
fname = "/home/jbushart/sshfs/dnscaptures/measurements/misclassifications-ow-small.json"
fname = "/home/jbushart/projects/encrypted-dns/results/2019-02-11-ow-from-cw/misclassifications-ow-pre-k7.json.xz"
content = []
with lzma.open(fname, "rb") as f:
    # with open(fname, 'rb') as f:
    lines = iter(f)
    for line in lines:
        try:
            content.append(json.loads(line))
        except Exception:
            # Catch parse errors
            pass

# %%
def is_class_correct(
    entry: t.Dict[str, t.Any]  # pylint: disable=redefined-outer-name
) -> bool:
    highest_count = 0
    for x in entry["class_result"]["options"]:
        highest_count = max(highest_count, x["count"])
    smallest_dist = 9_999_999_999
    for x in entry["class_result"]["options"]:
        if x["count"] == highest_count:
            smallest_dist = min(smallest_dist, int(x["distance_min"]))

    options = [
        x
        for x in entry["class_result"]["options"]
        if x["count"] == highest_count and int(x["distance_min"]) == smallest_dist
    ]
    # Only correct if there is exactly one options, and that one is correct
    return len(options) == 1 and options[0]["name"] == entry["label"]


# %% [markdown]
# # Most common misclassifications

# %%
# most_common_misclassification: t.Counter[str] = Counter()
# for entry in content:
#     if entry["reason"] is not None:
#         continue
#     for mis in entry["class_result"]["options"]:
#         if int(mis["distance_min"]) < 100000:
#             most_common_misclassification.update([mis["name"]] * mis["count"])

# %%
# most_common_misclassification.most_common(20)

# %%
# show_infos_for_domain("qichacha.com")

# %% [markdown]
# # Minimal / Maximal distance within block of misclassifications

# %%
# # Load all local dnstap files with the current pylib version
# # The current pylib version might be newer and have more classifications
# local_root = "/home/jbushart/projects/data/dnscaptures-open-world/"
# # index by filename
# cache = {}
# for _, seqs in pylib.load_folder(local_root):
#     for seq in seqs:
#         cache[Path(seq.id()).name] = seq

# local_root = "/home/jbushart/projects/data/dnscaptures-main-group/"
# for _, seqs in pylib.load_folder(local_root):
#     for seq in seqs:
#         cache[Path(seq.id()).name] = seq

# %%
# for each k, count min and max distances, as well as sequence lengths
# Map "k" -> "file name" -> Stats
distances_per_k: t.Dict[int, t.Dict[str, SequenceStats]] = {}
for entry in content:
    # These two domains have broken sequences (SERVFAIL in CF responses)
    # and their distances are wrong
    if entry["label"] in ["szedu.net", "tedu.cn"]:
        continue

    d = distances_per_k.setdefault(entry["k"], {})

    # current sequences length
    fname = Path(entry["id"]).name
    #     curr_length: int = cache[fname].len()
    #
    #     if cache[fname].classify():
    #         continue

    if entry["reason"]:
        continue
    if is_class_correct(entry):
        continue

    # Get min and max distance
    min_dist = 999_999.0
    min_dist_label = ""
    max_dist = 0.0
    max_dist_label = ""
    for mis in entry["class_result"]["options"]:
        #         if int(mis["distance_min"]) < min_dist:
        #             min_dist = int(mis["distance_min"])
        #             min_dist_label = mis["name"]
        #         if int(mis["distance_max"]) > max_dist:
        #             max_dist = int(mis["distance_max"])
        #             max_dist_label = mis["name"]
        if float(mis["distance_min_norm"]) < min_dist:
            min_dist = float(mis["distance_min_norm"])
            min_dist_label = mis["name"]
        if float(mis["distance_max_norm"]) > max_dist:
            max_dist = float(mis["distance_max_norm"])
            max_dist_label = mis["name"]

    #     stats = SequenceStats(curr_length, min_dist, min_dist_label, max_dist, max_dist_label)
    stats = SequenceStats(0, min_dist, min_dist_label, max_dist, max_dist_label)
    d[fname] = stats

# %%
min_dists_norm = {
    key: sorted([float(stat.min_dist) for stat in stats_.values()])
    for key, stats_ in distances_per_k.items()
}
max_dists_norm = {
    key: sorted([float(stat.max_dist) for stat in stats_.values()])
    for key, stats_ in distances_per_k.items()
}

# %%
# min_dists_norm = {
#     key: sorted(
#         [float(stat.min_dist)/stat.seq_length for stat in stats_.values()]
#     )
#     for key, stats_ in distances_per_k.items()
# }
# max_dists_norm = {
#     key: sorted(
#         [float(stat.max_dist)/stat.seq_length for stat in stats_.values()]
#     )
#     for key, stats_ in distances_per_k.items()
# }

# %%
plt.plot(range(len(min_dists_norm[7])), min_dists_norm[7], label="Min")
for key, values in list(max_dists_norm.items())[1:]:
    plt.plot(range(len(values)), values, label=f"Max k={key}")
plt.ylim(0, 4)
# plt.hlines(y=2.16, xmin=0, xmax=200000, color='black')
plt.gcf().set_size_inches(15, 10)
plt.legend()
plt.tight_layout()
plt.xlim(0, 82490)

# %%
len(min_dists_norm[7])

# %%
# Try to figure out how the thresholds have to be to set a false positive rate of 10%, 20%, etc.
dists = sorted(min_dists_norm[7])
total_count_without_prefilter = 82490
for i in range(0, 101, 5):
    idx = total_count_without_prefilter * i // 100
    if idx < len(dists):
        #         print(f"{i:>3}%: {dists[idx]}")
        print(f"\\percent{{{i}}} & {dists[idx]} \\\\")
#         print(f"env RAYON_NUM_THREADS=20 RUST_LOG=dns_sequence=info ./dns-sequence-final -d ./redirects.csv ./confusion_domains.csv --exact-k=7 --misclassifications=./misclassifications-fpr-{i}.json --statistics=statistics-fpr-{i}.csv /mnt/data/dnscaptures-main-group crossvalidate --dist-thres={dists[idx]} | tee dns-sequence-final-fpr-{i}.log")

# %%
# Count how many traces will not match due to the distance threshold
for k, values in max_dists_norm.items():
    print(k, sum([1 for v in values if v <= 2.16]))

# %%
# Values which are too extreme
for fname, stat in distances_per_k[7].items():
    if float(stat.min_dist) / stat.seq_length >= 6:
        print(fname, stat)

# %%
