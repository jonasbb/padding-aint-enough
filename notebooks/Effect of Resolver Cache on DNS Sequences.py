# -*- coding: utf-8 -*-
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

# %% [markdown]
# # Effect of Resolver Cache on DNS Sequences
#
# Here we want to see, if and how the cache of the recursive resolver affects the DNS Sequences.
# The initial working assumption is, that if the resolver starts with an empty cache which gets warm because of the experiments, then the initial and the final DNS Sequence might differ in their timings.

# %%
import typing as t

import matplotlib.pyplot as plt
import pylib
import seaborn as sns

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [15, 10]

# %%
sequences = pylib.load_folder("/mnt/data/Downloads/dnscaptures-main-group")

# %%
USE_NORMALIZE_DISTANCES = True

# %%
distances_between_consecutive_sequences: t.Dict[int, t.List[int]] = dict()

for domain, seqs in sequences:
    for i in range(9):
        a, b = seqs[i : i + 2]
        a_len, b_len = a.len(), b.len()
        dist = a.distance(b)
        dist_norm = dist / max(a_len, b_len)
        d = dist_norm if USE_NORMALIZE_DISTANCES else dist
        distances_between_consecutive_sequences.setdefault(i, list()).append(d)

# %%
sns.set(style="white", palette="muted", color_codes=True)
plots = [[0, 1, 8], [0, 1, 7], [0, 1], [0, 7], [0, 8]]
for toplot in plots:
    for i in toplot:
        sns.distplot(
            distances_between_consecutive_sequences[i],
            hist=False,
            kde_kws={"shade": True},
            label=f"{i} → {i+1}",
        )
    if USE_NORMALIZE_DISTANCES:
        plt.title("Normalized Distances")
        plt.xlim(left=0, right=4)
    else:
        plt.title("Absolute Distances")
        plt.xlim(left=0, right=600)
    plt.savefig(
        f"resolver-cache-consecutive-sequences-{toplot}-{USE_NORMALIZE_DISTANCES}.svg"
    )
    plt.show()

# %%
