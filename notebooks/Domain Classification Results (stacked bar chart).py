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

from common_functions import autolabel, label2good_label


# %%
def parse_log_data(fname: str) -> t.List[t.List[t.Tuple[str, t.List[int]]]]:
    """
    Returned the parsed data of the domain classification results from the log file

    The Data is structured like:
    * The outer list contains data per `k` value, in the order of the log file
    * There is only Tuple, per result quality
    * The list in the tuple are the number of domains for the given result quality
    """
    with open(fname) as f:
        content = f.read()
    # This marks the start of the table we are interested in
    separator = (
        "#Domains with at least x classification results of quality or higher:\n"
    )
    # Drop everyting before the first table
    datas = content.split(separator)[1:]
    res = []
    for data in datas:
        # Only keep the lines we are interested in of the table
        lines = data.splitlines()[3:7]
        tmp = []
        for line in lines:
            elements = [x.strip() for x in line.split("│")]
            quality = elements[0]
            values = [int(x) for x in elements[1:]]
            assert (
                len(values) == 11 or len(values) == 21
            ), f"Values must be 11 or 21 entries long but is only {len(values)}. For n/10 domains and n 0 to 10 (inclusive)."
            tmp.append((quality, values))
        tmp = tmp[::-1]
        res.append(tmp)
    return res


# %%
# Use this for a single file
pdatas = parse_log_data("../results/2019-02-09-cache-both/dns-sequence-cache-both.log")
pdatas_err: t.Optional[t.List[t.List[float]]] = None

# # Use this when plotting the average of two files
# # Parse all the data
# pdatas_a = parse_log_data("../results/2019-02-04-scenario4/scenario4.log")
# pdatas_b = parse_log_data("../results/2019-02-04-scenario4/scenario4-b.log")
# pdatas = [
#     [
#         (aa[0], [aaa + bbb for aaa, bbb in zip(aa[1], bb[1])])
#         # Iterate over the different qualities
#         for aa, bb in zip(a, b)
#     ]
#     # Iterate over the different k's
#     for a, b in zip(pdatas_a, pdatas_b)
# ]
# # Sum all qualities per k for both a and b
# _a = [
#     [sum(x) for x in zip(*[z[1] for z in a])]
#     # Iterate over the different k's
#     for a in pdatas_a
# ]
# _b = [
#     [sum(x) for x in zip(*[z[1] for z in b])]
#     # Iterate over the different k's
#     for b in pdatas_b
# ]
# pdatas_err: t.Optional[t.List[t.List[float]]] = [[abs(aa - bb) / 2 for aa, bb in zip(a, b)] for a, b in zip(_a, _b)]

# %%
for i, pdata in enumerate(pdatas):
    colors = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
    hatches = cycle(["/", "-", "\\", "|"])

    plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})

    prev_values = [0] * 20
    total_domains = pdata[0][1][0]
    for label, values in pdata:
        label = label2good_label(label)
        # skip the 0/10 case as not relevant
        values = values[1:]
        print(values)
        heights = [v - pv for v, pv in zip(values, prev_values)]
        if total_domains == 9207:
            total_domains = 9205
        # Convert into percentages
        h = [v * 100 / total_domains for v in heights]
        ph = [v * 100 / total_domains for v in prev_values]

        kwargs: t.Dict[str, t.Any] = {}
        # Plot error bars, if available
        if pdatas_err and "Pseudo" in label:
            kwargs["yerr"] = [
                v * 100 / total_domains
                for v in pdatas_err[i][1:]  # pylint: disable=unsubscriptable-object
            ]
            kwargs["error_kw"] = {"lw": 5}

        bars = plt.bar(
            range(1, 1 + len(values)),
            h,
            bottom=ph,
            label=label,
            width=1.01,
            color=next(colors),
            hatch=next(hatches),
            **kwargs,
        )
        prev_values = values

    yoffset = None
    if pdatas_err:
        yoffset = [
            v * 100 / total_domains
            for v in pdatas_err[i][1:]  # pylint: disable=unsubscriptable-object
        ]
    autolabel(bars, plt, yoffset=yoffset, precision=0)

    plt.gcf().set_size_inches(7, 4)

    plt.ylim(0, 100)
    #     plt.xlim(0.5, 10.5)
    plt.xlim(0.5, 20.5)
    # CAREFUL: Those are tiny spaces around the /
    plt.xticks(range(1, 21), [f"{i}" for i in range(1, 21)])
    plt.ylabel("Percent of all domains")
    plt.xlabel("At least n / 20 domains correctly classified")

    #     plt.legend(loc="upper right", bbox_to_anchor=(1, 1), borderpad=0, frameon=False)
    plt.legend(loc="lower left", bbox_to_anchor=(0, 0), frameon=True)
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-k{i*2 + 1}.svg")
    plt.show()

# %%
# Parse all the data
pdatas = [
    parse_log_data(
        f"../results/2019-02-09-ow-small/dns-sequence-ow-small-fpr-{fpr}.log"
    )[0]
    for fpr in range(10, 91, 10)
]

# %%
# Create a structure similar to pdata, but where the entries are organized by n/10
data_per_n_ = []
for i in range(len(pdatas[0][0][1])):
    tmp: t.Dict[str, t.List[int]] = {}
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
        print(values)
        heights = [v - pv for v, pv in zip(values, prev_values)]
        # Convert into percentages
        h = [v * 100 / 9207 for v in heights]
        ph = [v * 100 / 9207 for v in prev_values]
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
    plt.xlim(0.5, 9.5)

    plt.legend(loc="upper center", ncol=4, mode="expand")
    #     plt.legend(loc="lower center", ncol=4, mode="expand")
    plt.gcf().set_size_inches(7, 4)
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-k7-n{i}.svg")
    plt.show()

# %%
