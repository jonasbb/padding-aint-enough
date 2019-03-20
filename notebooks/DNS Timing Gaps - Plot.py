# -*- coding: utf-8 -*-
# ---
# jupyter:
#   jupytext:
#     formats: ipynb,auto:percent
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
# pylint: disable=pointless-statement

# %%
import csv
import math
import typing as t
from collections import Counter

import matplotlib.pyplot as plt

# %%
# %matplotlib inline
# # %matplotlib notebook

# %%
def human_time(duration_in_nano_seconds: int) -> str:
    if duration_in_nano_seconds < 1000:
        return f"{duration_in_nano_seconds}ns"
    duration_in_micro_seconds = duration_in_nano_seconds // 1000
    if duration_in_micro_seconds < 1000:
        return f"{duration_in_micro_seconds}µs"
    duration_in_milli_seconds = duration_in_micro_seconds // 1000
    if duration_in_milli_seconds < 1000:
        return f"{duration_in_milli_seconds}ms"
    duration_in_seconds = duration_in_milli_seconds // 1000
    if duration_in_seconds < 3600:
        return f"{duration_in_seconds}s"
    duration_in_hours = duration_in_seconds // 3600
    return f"{duration_in_hours}h"


# %%
content = [(int(gap), int(count)) for (gap, count) in csv.reader(open("gaps.csv"))]

# %%
counts: t.Counter[int] = Counter()
for gap, count in content:
    gap = int(math.log2(gap)) if gap > 0 else 0
    counts[gap] += count

# %%
counts

# %%
plt.plot(counts.keys(), counts.values())
plt.gcf().set_size_inches(12, 6.75)
plt.tight_layout()
xticks = range(0, max(counts.keys()) + 1)
plt.xlim(0, max(counts.keys()))
plt.ylim(0, plt.ylim()[1])
xticks_labels = [human_time((2 ** i) * 1000) for i in xticks]
_ = plt.xticks(xticks, xticks_labels, rotation="20")
plt.savefig("gaps_distribution.svg")

# %%



# %%

