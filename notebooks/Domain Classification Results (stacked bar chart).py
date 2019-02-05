# -*- coding: utf-8 -*-
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
# %matplotlib inline

# %%
import typing as t
from itertools import cycle

import matplotlib.cm
import matplotlib.pyplot as plt

from common_functions import autolabel


# %%
def label2good_label(label: str) -> str:
    if label == "PluralityThenMinDist":
        return "Pseudo-Plurality\n + Distance"
    return label


# %%
def parse_data(data: str) -> t.List[t.Tuple[str, t.List[int]]]:
    res = []
    for line in data.strip().split("\n"):
        elements = [x.strip() for x in line.split("│")]
        quality = elements[0]
        values = [int(x) for x in elements[1:]]
        assert (
            len(values) == 11
        ), "Values must be 11 entries long. For n/10 domains and n 0 to 10 (inclusive)."
        res.append((quality, values))
    return res[::-1]


# %%
datas = [
    """
 PluralityThenMinDist │ 9205 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Plurality            │ 9205 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Majority             │ 9205 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Exact                │ 9205 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390

""",
    """
 PluralityThenMinDist │ 9205 │ 8587 │ 8397 │ 8081 │ 7718 │ 7241 │ 6711 │ 6054 │ 5222 │ 4162 │ 2628
 Plurality            │ 9205 │ 8217 │ 7853 │ 7434 │ 6955 │ 6397 │ 5827 │ 5179 │ 4339 │ 3358 │ 2050
 Majority             │ 9205 │ 8217 │ 7853 │ 7434 │ 6955 │ 6397 │ 5827 │ 5179 │ 4339 │ 3358 │ 2050
 Exact                │ 9205 │ 6998 │ 6206 │ 5528 │ 4956 │ 4366 │ 3731 │ 3088 │ 2436 │ 1752 │  924

""",
    """
 PluralityThenMinDist │ 9205 │ 8588 │ 8379 │ 8094 │ 7762 │ 7325 │ 6767 │ 6163 │ 5398 │ 4320 │ 2837
 Plurality            │ 9205 │ 8387 │ 8025 │ 7669 │ 7238 │ 6742 │ 6141 │ 5496 │ 4675 │ 3704 │ 2354
 Majority             │ 9205 │ 7774 │ 7249 │ 6737 │ 6253 │ 5717 │ 5113 │ 4456 │ 3730 │ 2879 │ 1776
 Exact                │ 9205 │ 5134 │ 4241 │ 3596 │ 3031 │ 2598 │ 2172 │ 1765 │ 1363 │  926 │  477

""",
    """
 PluralityThenMinDist │ 9205 │ 8573 │ 8375 │ 8079 │ 7768 │ 7349 │ 6822 │ 6209 │ 5430 │ 4405 │ 2893
 Plurality            │ 9205 │ 8382 │ 8044 │ 7677 │ 7284 │ 6786 │ 6210 │ 5574 │ 4757 │ 3806 │ 2440
 Majority             │ 9205 │ 7187 │ 6583 │ 6041 │ 5551 │ 5073 │ 4484 │ 3922 │ 3222 │ 2487 │ 1535
 Exact                │ 9205 │ 3607 │ 2726 │ 2153 │ 1778 │ 1454 │ 1196 │  976 │  740 │  528 │  271

""",
    """
 PluralityThenMinDist │ 9205 │ 8539 │ 8332 │ 8044 │ 7724 │ 7308 │ 6788 │ 6198 │ 5428 │ 4415 │ 2914
 Plurality            │ 9205 │ 8338 │ 8001 │ 7636 │ 7262 │ 6743 │ 6177 │ 5530 │ 4749 │ 3810 │ 2465
 Majority             │ 9205 │ 6582 │ 5911 │ 5337 │ 4886 │ 4409 │ 3939 │ 3425 │ 2810 │ 2135 │ 1331
 Exact                │ 9205 │ 2320 │ 1645 │ 1269 │ 1041 │  815 │  662 │  553 │  411 │  285 │  149

""",
]

# %%
for i, data in enumerate(datas):
    colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
    hatches = cycle(["/", "-", "\\", "|"])

    plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})

    prev_values = [0] * 10
    pdata = parse_data(data)
    total_domains = pdata[0][1][0]
    for label, values in pdata:
        label = label2good_label(label)
        # skip the 0/10 case as not relevant
        values = values[1:]
        heights = [v - pv for v, pv in zip(values, prev_values)]
        # Convert into percentages
        h = [v * 100 / total_domains for v in heights]
        ph = [v * 100 / total_domains for v in prev_values]
        bars = plt.bar(
            range(1, 1 + len(values)),
            h,
            bottom=ph,
            label=label,
            width=1.01,
            color=next(colors),
            hatch=next(hatches),
        )
        prev_values = values
    autolabel(bars, plt)

    # print values for k=7 extra
    if i * 2 + 1 == 7:
        for v in prev_values:
            print(v * 100 / total_domains)

    plt.gcf().set_size_inches(7, 4)

    plt.ylim(0, 100)
    plt.xlim(0.5, 10.5)
    # CAREFUL: Those are tiny spaces around the /
    plt.xticks(range(1, 11), [f"{i} / 10" for i in range(1, 11)])
    plt.ylabel("Percent of all domains")
    plt.xlabel("At least n / 10 domains correctly classified")

    plt.legend(loc="upper right", bbox_to_anchor=(1, 1), borderpad=0, frameon=False)
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-k{i*2 + 1}.svg")
    plt.show()

# %%
# Parse all the data
pdatas = [parse_log_data(data) for data in datas]

# %%
# Create a structure similar to pdata, but where the entries are organized by n/10
data_per_n_ = []
for i in range(len(pdatas[0][0][1])):
    tmp = {}
    for quality, _ in pdatas[0]:
        tmp[quality] = []
    data_per_n_.append(tmp)

for i in range(len(pdatas[0][0][1])):
    for pdata in pdatas:
        for quality, values in pdata:
            data_per_n_[i][quality].append(values[i])

# Convert the Dict into a tuple
data_per_n = [[(q, v) for q, v in x.items()] for x in data_per_n_]
del data_per_n_

# %%
for i, pdata in enumerate(data_per_n):
    colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
    hatches = cycle(["/", "-", "\\", "|"])

    plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})

    prev_values = [0] * len(pdata[0][1])
    total_domains = data_per_n[0][0][1][0]
    for label, values in pdata:
        label = label2good_label(label)
        heights = [v - pv for v, pv in zip(values, prev_values)]
        # Convert into percentages
        h = [v * 100 / 9205 for v in heights]
        ph = [v * 100 / 9205 for v in prev_values]
        bars = plt.bar(
            range(1, 1 + len(values)),
            h,
            bottom=ph,
            label=label,
            width=1.01,
            color=next(colors),
            hatch=next(hatches),
        )
        prev_values = values
    autolabel(bars, plt)

    # CAREFUL: Those are tiny spaces around the =
    #     plt.xticks(range(1, 6), [f"k = {i}" for i in range(1, 10, 2)])
    plt.xticks(range(1, 11), [f"{i}0%" for i in range(1, 11)])
    plt.xlabel("FPR in OW")
    plt.ylabel("Percent of all domains")
    plt.title(f"At least {i} / 10 domains correctly classified")

    plt.ylim(0, 100)
    plt.xlim(0.5, 10.5)

    plt.legend(loc="upper center", ncol=4, mode="expand")
    plt.gcf().set_size_inches(7, 4)
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-k7-n{i}.svg")
    plt.show()


# %%
