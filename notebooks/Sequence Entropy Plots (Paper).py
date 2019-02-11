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
import json
import lzma
import math
import typing as t
from collections import Counter

import matplotlib.pyplot as plt

from common_functions import COLORS, HATCHES

# %%
colors = [c for _, c in zip(range(9), COLORS)]
hatches = [c for _, c in zip(range(5), HATCHES)]

# %%
# Map from filename to tuple of entropy and result quality
data = json.load(lzma.open("./sequences-stats-cw.json.xz", "rt"))

# %% [markdown]
# # First count how many files there are in each category of entropy

# %%
def filter_out(
    entropy: t.Any  # pylint: disable=unused-argument,redefined-outer-name
) -> bool:
    #     # VERY good
    #     if entropy["length"] <= 6:
    #         return True
    return False


# %%
# EntropyKind -> EntropyValue -> Counter of ResultQuality
counters_per_entropy: t.Dict[str, t.Dict[float, t.Counter[str]]] = {}

# Create initial key/value entries
for keys in next(iter(data.values()))[0].keys():
    counters_per_entropy[keys] = {}

filtered_out: t.Counter[t.Tuple[float, str]] = Counter()

# Count files now
for entropy, result_quality in data.values():
    # Skip some values and do not put them into the graphs below
    if filter_out(entropy):
        filtered_out.update([(entropy["count_messages"], result_quality)])
        continue

    for entropy_kind, value in entropy.items():
        # Some values might be `None`, because they are not computable.
        # For example: n-grams (n>1) for Sequences of length 1
        # Simply assume no entropy for them
        if value is None:
            value = 0

        counter = counters_per_entropy[entropy_kind].setdefault(value, Counter())
        counter.update([result_quality])

# %%
# Hardcoded list of all result qualities and if they are good (meaning we use them in the paper) or bad
good_result_qualities = ["Exact", "Majority", "Plurality", "PluralityThenMinDist"]
bad_result_qualities = ["Contains", "Wrong", "NoResult"]

# %%
# Provide some limits for for wide the plots should be based on knowledge of the plots
plot_args: t.Dict[str, t.Dict[str, t.Any]] = {}
plot_args["length"] = {}
plot_args["complexity"] = {}
plot_args["count_messages"] = {}
plot_args["shannon_n1"] = {}
plot_args["shannon_n2"] = {}
plot_args["shannon_n3"] = {}

plot_args["count_messages"]["xlim"] = (1, 200)
plot_args["shannon_n1"]["xlim"] = (0.28, 0.35)
plot_args["shannon_n2"]["xlim"] = (0, 0.09)
plot_args["shannon_n3"]["xlim"] = (0, 0.055)

plot_args["length"]["ylim"] = (0, 1750)
plot_args["complexity"]["ylim"] = (0, 2250)
plot_args["count_messages"]["ylim"] = (0, 2250)
plot_args["shannon_n1"]["ylim"] = (0, 425)
plot_args["shannon_n2"]["ylim"] = (0, 750)
plot_args["shannon_n3"]["ylim"] = (0, 1000)

# %% [markdown]
# # Plot Entropy and Sequence count
#
# * Blue: Number of sequences with exact entropy classified wrongly.
#
#     Left Axis
# * Orange: Number of sequences with exact entropy classified correctly.
#
#     Left Axis
#
# * Red: Fraction of sequences with entropy smaller-equal classified wronly.
#
#     Right Axis
# * Green: Fraction of sequences with entropy smaller-equal classified correctly.
#
#     Right Axis
# * Grey/Black: Percentage of sequences having an entropy smaller-equal out of the total count.
#     This shows to how many sequences the red/green part applies.
#
#     Right Axis
#
# If the title contains **(Reverse Cumsum)**, then the Red/Green/Grey parts change their meaning slightly.
# Instead of referring to all sequences with an entropy smaller-equal, they refer to all sequences larger-equal.
# This provides the information "How well would classification work, if we exclude low entropy sequences?".

# %%
# Specify how many digits precision, after the decimal point, should be used
# e.g.: 3 -> 0.###
shannon_resolution = 4

float2str = lambda v: f"{v:.{shannon_resolution}f}"

# %%
plt.rcParams.update({"legend.handlelength": 3, "legend.handleheight": 1.5})
for title, x in counters_per_entropy.items():
    if "shannon" in title:
        # Only create the x-values as far as needed for the xlim
        # Hopefully this saves some plotting overhead
        xlim = plot_args.get(title, {}).get("xlim", (0, 1.0))
        # Create all fractional values between 0 and 1 with `shannon_resolution` of precision
        xmax = 10 ** shannon_resolution
        xs = [x / xmax for x in range(xmax) if (x / (xmax)) <= xlim[1]]
        # Since there is no easy way to get a float with a precision of 3-decimal places behind the comma
        # the solution here is to convert the float into a string with the desired precision.
        # The `ys_tmp` maps each precision-string to a Counter
        # We iterate over the dict, by taking the values stored in `xs` and converting them
        # to the string in the same way
        ys_tmp: t.Dict[str, t.Counter[str]] = {}
        # Limit precision of entropy
        for entropy, counter in x.items():
            c = ys_tmp.setdefault(float2str(entropy), Counter())
            c += counter
        ys_good = [
            sum(
                ys_tmp.get(float2str(v), Counter()).get(quality, 0)
                for quality in good_result_qualities
            )
            for v in xs
        ]
        ys_bad = [
            sum(
                ys_tmp.get(float2str(v), Counter()).get(quality, 0)
                for quality in bad_result_qualities
            )
            for v in xs
        ]
    else:
        # These only work, if the values are integer values
        xs = list(range(1, 1 + math.ceil(max(x.keys())), 1))
        ys_good = [
            sum(
                x.get(v, Counter()).get(quality, 0) for quality in good_result_qualities
            )
            for v in xs
        ]
        ys_bad = [
            sum(x.get(v, Counter()).get(quality, 0) for quality in bad_result_qualities)
            for v in xs
        ]
    ys = [a + b for a, b in zip(ys_good, ys_bad)]

    # Store some generic arguments for all plt.bar() calls
    kwargs: t.Dict[str, t.Any] = {}
    kwargs["step"] = "pre"
    kwargs["linewidth"] = 0.0
    #         if "shannon" in title:
    #             kwargs["width"] = 1 / (10 ** shannon_resolution)
    #         else:
    #             kwargs["width"] = 1

    ln1 = plt.fill_between(
        xs, ys_bad, **kwargs, facecolor=colors[0], hatch=hatches[0], label="Wrong"
    )
    ln2 = plt.fill_between(
        xs,
        [yg + yb for yg, yb in zip(ys_good, ys_bad)],
        **kwargs,
        y2=ys_bad,
        facecolor=colors[2],
        hatch=hatches[2],
        label="Correct",
    )

    kwargs = {}

    ylim = plot_args.get(title, {}).get("ylim", None)
    if ylim:
        plt.ylim(ylim)

    plt.ylabel("#DNS Sequences")
    plt.xlabel("Count of DNS replies")

    # We need a second axis to plot percentages
    ax2 = plt.gca().twinx()

    fraction = [
        good * 100 / (good + bad) if good > 0 else 0
        for good, bad in zip(ys_good, ys_bad)
    ]
    # smooth fraction a bit
    fraction = [sum(x) / len(x) for x in zip(fraction, fraction[1:])]
    ax2.step(
        xs[: len(fraction)],
        fraction,
        **kwargs,
        color="white",
        where="pre",
        label="Ratio",
        linewidth=3,
    )
    ln3 = ax2.step(
        xs[: len(fraction)],
        fraction,
        **kwargs,
        color="black",
        where="pre",
        label="Ratio",
        linewidth=1,
    )
    ax2.set_ylim(0, 100)
    ax2.set_ylabel("% correctly classified")

    xlim = plot_args.get(title, {}).get("xlim", (0, 250))
    print(plt.xlim(xlim))

    plt.gcf().set_size_inches(7, 4)

    lns = [ln2, ln1, *ln3]
    labs = [l.get_label() for l in lns]
    plt.legend(lns, labs, loc="center right")
    plt.tight_layout()
    plt.savefig(f"sequence-entropy-{title}.svg")
    plt.show()
    plt.close()

# %%
