# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.3'
#       jupytext_version: 1.3.4
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
from common_functions import autolabel, label2good_label, parse_log_data

# %%
pdatas_err: t.Optional[t.List[t.List[float]]] = None

# Use this for a single file
# pdatas = parse_log_data("../results/2019-02-09-cache-both/dns-sequence-cache-both.log")
# pdatas = parse_log_data("../results/2019-01-09-closed-world/dns-sequence-final.log")
pdatas = parse_log_data(
    "../results/2019-11-18-full-rescan/crossvalidate/crossvalidate-res-0.log.xz"
)


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

    prev_values = [0] * (len(pdata[0][1]) - 1)
    total_domains = pdata[0][1][0]
    for label, values in pdata:
        label = label2good_label(label)
        # skip the 0/10 case as not relevant
        values = values[1:]
        print(values)
        heights = [v - pv for v, pv in zip(values, prev_values)]
        # If all heights are 0, then we do not need to draw anything for this step
        if sum(heights) == 0:
            continue
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
    precision = 1
    if len(prev_values) > 15:
        precision = 0
    autolabel(bars, plt, yoffset=yoffset, precision=precision)

    plt.gcf().set_size_inches(7, 4)

    plt.ylim(0, 100)
    plt.xlim(0.5, len(prev_values) + 0.5)
    # CAREFUL: Those are tiny spaces around the /
    plt.xticks(
        range(1, len(prev_values) + 1), [f"{i}" for i in range(1, len(prev_values) + 1)]
    )
    plt.ylabel("Correctly classified websites in %")
    plt.xlabel(f"At least n / {len(prev_values)} traces correctly classified")

    # plt.legend(loc="upper right", bbox_to_anchor=(1, 1), borderpad=0, frameon=False)
    plt.legend(loc="lower left", bbox_to_anchor=(0, 0), frameon=True)
    #     plt.legend(loc="upper center", ncol=4, mode="expand")
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-k{i*2 + 1}.svg")
    plt.show()

# %%
# Parse all the data
pdatas = [
    parse_log_data(
        # f"../results/2019-02-11-ow-from-cw/dns-sequence-final-fpr-{fpr}.log"
        # f"../results/2019-02-09-ow-small/dns-sequence-ow-small-fpr-{fpr}.log"
        f"../results/2019-11-18-full-rescan/fpr/res-fpr-{fpr}pc.log"
    )[0]
    for fpr in range(5, 91, 5)
]
total_of_sequences = pdatas[0][0][1][0]

# %%
# Create a structure similar to pdatas, but where the entries are organized by n/10
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
# This is to make a plot with the number of traces per FPR instead of the number of domains with x/10
# Simply hard code all values for now
data_per_n = [
    [
        (
            "Exact",
            [
                60286,
                71798,
                75027,
                76318,
                77037,
                77489,
                77923,
                78204,
                78416,
                78603,
                78776,
                78906,
                79033,
                79116,
                79180,
                79271,
                79372,
                79487,
            ],
        )
    ]
]
total_of_sequences = 92350

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
        # Skip tie breaking steps which never occur
        if sum(heights) == 0:
            continue
        # Convert into percentages
        h = [v * 100 / total_of_sequences for v in heights]
        ph = [v * 100 / total_of_sequences for v in prev_values]
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
    precision = 1
    if len(pdatas) > 15:
        precision = 0
    autolabel(bars, plt, precision=precision)

    # CAREFUL: Those are tiny spaces around the =
    #     plt.xticks(range(1, 6), [f"k = {i}" for i in range(1, 10, 2)])
    xlabels = [f"{i*5}" for i in range(1, len(pdatas) + 1)]
    plt.xticks(range(1, 1 + len(xlabels)), xlabels)
    plt.xlabel("False Positive Rate in %")
    plt.ylabel("Correctly classi-\nfied websites in %")
    plt.ylabel("True Positives in %")
    # plt.title(f"At least {i} / 10 traces correctly classified")

    plt.ylim(50, 100)
    plt.xlim(0.5, len(xlabels) + 0.5)

    # plt.legend(loc="upper center", ncol=4, mode="expand")
    # plt.legend(loc="lower center", ncol=4, mode="expand")
    # plt.legend(loc="lower center", ncol=4)
    plt.gcf().set_size_inches(7, 4)
    plt.tight_layout()
    plt.savefig(f"classification-results-per-domain-n{i}.svg")
    plt.show()

# %%
