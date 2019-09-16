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

# %%
import json

import matplotlib.pyplot as plt

# %%
plt.rcParams["figure.figsize"] = [7, 6]

# %%
data = json.load(open("./distances-per-cluster.json"))
misclass_ = [
    json.loads(x)
    for x in open("./misclassifications-final-closed-world.json").readlines()
]
misclass = {x["id"]: x for x in misclass_ if x["k"] == 1}
del misclass_

# %%
next(iter(misclass.values()))

# %%
len(misclass.values())

# %%
v = next(iter(misclass.values()))

# %%
misclass_distance = [
    int(x["distance_min"])
    for v in misclass.values()
    for x in v["class_result"]["options"]
    if v["label"] != x["name"]
]
misclass_distance_avg = sum(misclass_distance) / len(misclass_distance)
# misclass_distance_avg = sorted(misclass_distance)[int(len(misclass_distance)/2)]
print("Len:", len(misclass_distance))
print("Average:", misclass_distance_avg)

# %% [markdown]
# The structure of data is a many nested lists with:
#
# 1. List over all domain
# 2. List over all traces in domain
# 3. List of distances to all other traces in same domain
# 4. Pair of (Distance, Length of First Sequence, Length of Second Sequence)

# %%
clusterdistances = [
    [
        distance / max(length1, length2)
        #         distance
        for trace in domain
        for distance, length1, length2 in trace
    ]
    for domain in data
    if len(domain) > 0
]

# %%
# perc95clusterdistances = [sorted(distances)[:85] for distances in clusterdistances]

# %%
# sortedperc95clusterdistances = sorted(
#     perc95clusterdistances, key=lambda x: list(reversed(x))
# )

# %%
sortedclusterdistances = sorted(
    [sorted(distances) for distances in clusterdistances if len(distances) > 0],
    key=lambda x: list(reversed(x)),
)
plt.plot(list(x[-1] for x in sortedclusterdistances), label="Max. Distance per Domain")
plt.plot(list(x[0] for x in sortedclusterdistances), label="Min. Distance per Domain")
plt.plot(
    [misclass_distance_avg] * len(sortedclusterdistances),
    label="Avg. Distance for wrong Classification",
)
# plt.plot(sorted(misclass_distance), linewidth=10)
plt.xlim(0, len(list(sortedclusterdistances)))
plt.ylim(bottom=0)
plt.xlabel("Domains (sorted by Min. Distance)")
plt.ylabel("Distance between DNS sequences")
plt.legend()

# %%
len([1 for x in sortedclusterdistances2 if x[0] == 0])

# %%
sortedclusterdistances2 = sorted(
    [sorted(distances) for distances in clusterdistances if len(distances) > 0],
    key=list,
)
plt.plot(list(x[-1] for x in sortedclusterdistances2), label="Max. Distance per Domain")
plt.plot(
    list(x[0] for x in sortedclusterdistances2),
    label="Min. Distance per Domain",
    linewidth=2,
)
# plt.plot([misclass_distance_avg] * len(sortedclusterdistances), label="Avg. Distance for wrong Classification", linewidth=2, color="black")

plt.xlim(0, len(list(sortedclusterdistances2)))
plt.ylim(bottom=0)
plt.xlabel("Domains (sorted by Min. Distance)")
plt.ylabel("Distance between DNS sequences (normalized)")
plt.legend()
plt.tight_layout()
plt.savefig(f"distance-per-cluster-normalized.svg")

# %%
sortedclusterdistances2 = sorted(
    [sorted(distances) for distances in clusterdistances if len(distances) > 0],
    key=list,
)
plt.plot(list(x[0] for x in sortedclusterdistances2))
plt.plot(list(x[-1] for x in sortedclusterdistances))
plt.ylabel("Distance between DNS sequences")
plt.xlabel()

# %%

# %%
