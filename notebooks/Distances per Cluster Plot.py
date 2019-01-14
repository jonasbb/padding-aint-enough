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
# %matplotlib notebook

# %%
import json

import matplotlib.pyplot as plt

# %%
data = json.load(open("./distances-per-cluster.json"))

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
        for trace in domain
        for distance, length1, length2 in trace
    ]
    for domain in data
    if len(domain) > 0
]

# %%
#perc95clusterdistances = [sorted(distances)[:85] for distances in clusterdistances]

# %%
# sortedperc95clusterdistances = sorted(
#     perc95clusterdistances, key=lambda x: list(reversed(x))
# )

# %%
sortedclusterdistances = sorted(
    [sorted(distances) for distances in clusterdistances if len(distances) > 0],
    key=lambda x: list(reversed(x)),
)
plt.plot(list(x[-1] for x in sortedclusterdistances))
plt.plot(list(x[0] for x in sortedclusterdistances))

# %%
sortedclusterdistances = sorted(
    [sorted(distances) for distances in clusterdistances if len(distances) > 0],
    key=list,
)
plt.plot(list(x[0] for x in sortedclusterdistances))
plt.plot(list(x[-1] for x in sortedclusterdistances))
