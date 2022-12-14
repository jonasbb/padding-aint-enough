# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 1.2.0+dev
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
import typing as t
from collections import OrderedDict
from itertools import chain, combinations_with_replacement

import matplotlib.pyplot as plt
import pylib
from natsort import natsorted

# %%
K = t.TypeVar("K")
V = t.TypeVar("V")

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [25, 15]

# %%
s0 = pylib.load_file(
    "/mnt/data/Downloads/dnscaptures-main-group/google.com/google.com-0-0.dnstap.xz"
)
s1 = pylib.load_file(
    "/mnt/data/Downloads/dnscaptures-main-group/google.com/google.com-1-0.dnstap.xz"
)

# %%
s0

# %%
s0.distance(s1)

# %%
s0.distance_with_details(s1)


# %%
def merge_dict_counts(a: t.Dict[K, int], b: t.Dict[K, int]) -> t.Dict[K, int]:
    res = {}
    for key in chain(a.keys(), b.keys()):
        res[key] = a.get(key, 0) + b.get(key, 0)
    return res


# %%
def merge_costs_to_lists(l: t.Dict[K, t.List[V]], costs: t.Dict[K, V]) -> None:
    for key, value in costs.items():
        l.setdefault(key, list()).append(value)


# %%
def normalize_dict(d: t.Dict[K, int], r: int) -> t.Dict[K, float]:
    res = {}
    for key, value in d.items():
        res[key] = value / r
    return res


# %%
sequences = pylib.load_folder("/mnt/data/Downloads/dnscaptures-2019-09-06/extracted")

# %%
lists: t.Dict[str, t.List[float]] = {}
total = []
for domain, seqs in sequences[:]:
    for a, b in combinations_with_replacement(seqs, 2):
        l = max(a.len(), b.len())
        mc: t.Tuple[int, t.Dict[str, int]] = a.distance_with_details(b)
        costs = normalize_dict(mc[1], l)
        # Insert fake entries for all non-existing transitions
        for a in range(1, 15):
            for b in range(1, 15):
                costs.setdefault(f"gap({a})_to_gap({b})", 0)
        merge_costs_to_lists(lists, costs)
        total.append(mc[0] / l)
list_backup = lists
lists.keys(), len(total)

# %%
lists_distances = {
    key: value for key, value in list_backup.items() if "_to_" not in key
}
lists_counts = OrderedDict(
    {
        key: value
        for key, value in list_backup.items()
        # Only keep the entry if at least one value is not 0
        # If all values are 0 this means the key was generated artificially above
        # and carries no information about the DNS Sequences because this gap transformation was never seen
        if "_to_" in key and max(value) > 0
    }
)

# %%
lists = lists_distances
lists["total"] = total
labels = natsorted(list(lists.keys()))
values = [lists[l] for l in labels]
plt.plot([0, len(values) + 1], [0, 0], color="black", alpha=0.2)
plt.boxplot(values, labels=labels)
plt.ylim(bottom=-0.1, top=6)
plt.xticks(rotation=90)
plt.title("Normalized Distances")
plt.savefig(f"distance-cost-distribution-{len(total)}.svg")
plt.savefig(f"distance-cost-distribution-{len(total)}.png")
plt.show()

# %%
lists = lists_counts
labels = natsorted(list(lists.keys()))
values = [lists[l] for l in labels]
plt.plot([0, len(values) + 1], [0, 0], color="black", alpha=0.2)
plt.boxplot(values, labels=labels)
plt.ylim(bottom=-0.1)

plt.xticks(rotation=90)
plt.title("Normalized Distances")
plt.savefig(f"distance-cost-distribution-{len(total)}.svg")
plt.savefig(f"distance-cost-distribution-{len(total)}.png")
plt.show()

# %%
