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

T = t.TypeVar("T")

# %%
# %matplotlib inline
# # %matplotlib notebook

# %%
def human_time(duration_in_nano_seconds: float) -> str:
    duration_in_nano_seconds = int(duration_in_nano_seconds)
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
def sainte_lague(votes: t.Counter[T], total_seats: int) -> t.Counter[T]:
    """
    https://en.wikipedia.org/wiki/Webster/Sainte-Lagu%C3%AB_method
    """
    votes_and_seats = {party: [votes, 0.0] for party, votes in votes.items()}
    # For each seat we have to distribute, choose one party to distribute them to
    for _ in range(total_seats):
        highest_party = None
        highest_quotient = 0.0
        for party, (votescount, seats) in votes_and_seats.items():
            quotient = votescount / (2 * seats + 1)
            if quotient > highest_quotient:
                highest_party = party
                highest_quotient = quotient
        # Assign seat to party with highest quotient
        # This cast is valid, as highest_party will be set in the loop
        highest_party = t.cast(T, highest_party)
        votes_and_seats[highest_party][1] += 1
    return Counter({party: seats for party, (_votes, seats) in votes_and_seats.items()})


# %%
content = [(int(gap), int(count)) for (gap, count) in csv.reader(open("gaps.csv"))]

# %%
plt.plot([c[0] for c in content], [c[1] for c in content])
plt.gcf().set_size_inches(12, 6.75)
plt.tight_layout()
xmax = 1500000
xticks = range(0, xmax + 1, 100000)
plt.xlim(0, xmax)
plt.ylim(0, plt.ylim()[1])
plt.yscale("symlog", linthreshy=1)
xticks_labels = [human_time(i * 1000) for i in xticks]
_ = plt.xticks(xticks, xticks_labels, rotation="20")
plt.savefig("gaps.svg")

# %%
counts: t.Counter[int] = Counter()
for gap, count in content:
    #     gap = int(math.log2(gap)) if gap > 0 else 0
    gap = int(math.log(gap, math.sqrt(2))) if gap > 0 else 0
    counts[gap] += count
# set all other counts to 0
for key in range(max(counts.keys())):
    if key not in counts:
        counts[key] = 0
# Create a new, ordered Counter
tmp = counts
counts = Counter({key: tmp[key] for key in sorted(tmp.keys())})
del tmp

# %%
counts

# %%
plt.plot(counts.keys(), counts.values())
plt.gcf().set_size_inches(12, 6.75)
xticks = range(0, max(counts.keys()) + 1)
plt.xlim(0, max(counts.keys()))
plt.ylim(0, plt.ylim()[1])
# xticks_labels = [human_time((2 ** i) * 1000) for i in xticks]
xticks_labels = [human_time((math.sqrt(2) ** i) * 1000) for i in xticks]
_ = plt.xticks(xticks, xticks_labels, rotation="90")
plt.tight_layout()
plt.savefig("gaps_distribution.svg")

# %%
# for i in range(1, 200):
i = 1000
plt.close()
tmp = sainte_lague(counts, i)
plt.plot(tmp.keys(), tmp.values())
plt.gcf().set_size_inches(12, 6.75)
xticks = range(0, max(counts.keys()) + 1)
plt.xlim(0, max(counts.keys()))
plt.ylim(0, plt.ylim()[1])
# xticks_labels = [human_time((2 ** i) * 1000) for i in xticks]
xticks_labels = [human_time((math.sqrt(2) ** i) * 1000) for i in xticks]
_ = plt.xticks(xticks, xticks_labels, rotation="90")
plt.title(f"{i:0>3}")
plt.tight_layout()
# plt.savefig(f"gaps_distribution_sampled_{i:0>3}.png")

# %%
bursts_ = {
    int(length): int(count) for (length, count) in csv.reader(open("burst_lengths.csv"))
}
# Create sorted variant with missing keys set to 0
bursts = {
    length: bursts_.get(length, 0) for length in range(1, max(bursts_.keys()) + 1)
}
del bursts_


# %%
plt.plot(bursts.keys(), bursts.values())
plt.gcf().set_size_inches(12, 6.75)
plt.tight_layout()
xticks = range(0, 50 + 1, 10)
plt.xlim(1, max(xticks))
plt.ylim(0, plt.ylim()[1])
xticks_labels = [str(i) for i in xticks]
_ = plt.xticks(xticks, xticks_labels, rotation="20")
plt.savefig("bursts_distribution.svg")

# %%
