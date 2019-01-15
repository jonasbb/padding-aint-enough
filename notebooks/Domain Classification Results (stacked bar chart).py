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

import matplotlib.pyplot as plt


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
 PluralityThenMinDist │ 9207 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Plurality            │ 9207 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Majority             │ 9207 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390
 Exact                │ 9207 │ 8610 │ 8438 │ 8101 │ 7739 │ 7234 │ 6656 │ 5939 │ 5065 │ 3937 │ 2390

""",
    """
 PluralityThenMinDist │ 9207 │ 8587 │ 8397 │ 8081 │ 7718 │ 7241 │ 6711 │ 6054 │ 5222 │ 4162 │ 2628
 Plurality            │ 9207 │ 8217 │ 7853 │ 7434 │ 6955 │ 6397 │ 5827 │ 5179 │ 4339 │ 3358 │ 2050
 Majority             │ 9207 │ 8217 │ 7853 │ 7434 │ 6955 │ 6397 │ 5827 │ 5179 │ 4339 │ 3358 │ 2050
 Exact                │ 9207 │ 6998 │ 6206 │ 5528 │ 4956 │ 4366 │ 3731 │ 3088 │ 2436 │ 1752 │  924

""",
    """
 PluralityThenMinDist │ 9207 │ 8588 │ 8379 │ 8094 │ 7762 │ 7325 │ 6767 │ 6163 │ 5398 │ 4320 │ 2837
 Plurality            │ 9207 │ 8387 │ 8025 │ 7669 │ 7238 │ 6742 │ 6141 │ 5496 │ 4675 │ 3704 │ 2354
 Majority             │ 9207 │ 7774 │ 7249 │ 6737 │ 6253 │ 5717 │ 5113 │ 4456 │ 3730 │ 2879 │ 1776
 Exact                │ 9207 │ 5134 │ 4241 │ 3596 │ 3031 │ 2598 │ 2172 │ 1765 │ 1363 │  926 │  477

""",
    """
 PluralityThenMinDist │ 9207 │ 8573 │ 8375 │ 8079 │ 7768 │ 7349 │ 6822 │ 6209 │ 5430 │ 4405 │ 2893
 Plurality            │ 9207 │ 8382 │ 8044 │ 7677 │ 7284 │ 6786 │ 6210 │ 5574 │ 4757 │ 3806 │ 2440
 Majority             │ 9207 │ 7187 │ 6583 │ 6041 │ 5551 │ 5073 │ 4484 │ 3922 │ 3222 │ 2487 │ 1535
 Exact                │ 9207 │ 3607 │ 2726 │ 2153 │ 1778 │ 1454 │ 1196 │  976 │  740 │  528 │  271

""",
    """
 PluralityThenMinDist │ 9207 │ 8539 │ 8332 │ 8044 │ 7724 │ 7308 │ 6788 │ 6198 │ 5428 │ 4415 │ 2914
 Plurality            │ 9207 │ 8338 │ 8001 │ 7636 │ 7262 │ 6743 │ 6177 │ 5530 │ 4749 │ 3810 │ 2465
 Majority             │ 9207 │ 6582 │ 5911 │ 5337 │ 4886 │ 4409 │ 3939 │ 3425 │ 2810 │ 2135 │ 1331
 Exact                │ 9207 │ 2320 │ 1645 │ 1269 │ 1041 │  815 │  662 │  553 │  411 │  285 │  149

""",
]

# %%
for data in datas:
    prev_values = [0] * 10
    pdata = parse_data(data)
    for label, values in pdata:
        # skip the 0/10 case as not relevant
        values = values[1:]
        heights = [v - pv for v, pv in zip(values, prev_values)]
        plt.bar(range(1, 1 + len(values)), heights, bottom=prev_values, label=label)
        prev_values = values
    plt.gcf().set_size_inches(10, 7)
    plt.ylim(0, pdata[0][1][0])
    plt.legend()
    plt.tight_layout()
    plt.show()
