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
import math
import typing as t
from collections import Counter

import matplotlib.pyplot as plt
import numpy as np

# %%
# Map from filename to tuple of entropy and result quality
data = json.load(open("./sequences-stats-cw.json", "rt"))

# %% [markdown]
# # First count how many files there are in each category of entropy

# %%
# EntropyKind -> EntropyValue -> Counter of ResultQuality
counters_per_entropy: t.Dict[str, t.Dict[float, t.Counter[str]]] = {}

# Create initial key/value entries
for keys in next(iter(data.values()))[0].keys():
    counters_per_entropy[keys] = {}

# Count files now
for entropy, result_quality in data.values():
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
plot_args["shannon_n1"] = {}
plot_args["shannon_n1"]["xlim"] = (0, 0.6)
plot_args["shannon_n2"] = {}
plot_args["shannon_n2"]["xlim"] = (0, 0.15)
plot_args["shannon_n3"] = {}
plot_args["shannon_n3"]["xlim"] = (0, 0.15)

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

for do_reverse_cumsum in [False, True]:
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
                    x.get(v, Counter()).get(quality, 0)
                    for quality in good_result_qualities
                )
                for v in xs
            ]
            ys_bad = [
                sum(
                    x.get(v, Counter()).get(quality, 0)
                    for quality in bad_result_qualities
                )
                for v in xs
            ]
        ys = [a + b for a, b in zip(ys_good, ys_bad)]

        # Store some generic arguments for all plt.bar() calls
        kwargs: t.Dict[str, t.Any] = {}
        kwargs["align"] = "edge"
        if "shannon" in title:
            kwargs["width"] = 1 / (10 ** shannon_resolution)
        else:
            kwargs["width"] = 1

        plt.bar(xs, ys_bad, **kwargs)
        plt.bar(xs, ys_good, **kwargs, bottom=ys_bad)
        plt.ylabel("#Traces")

        xlim = plot_args.get(title, {}).get("xlim", (0, 250))
        plt.xlim(xlim)

        # We need a second axis to plot percentages
        ax2 = plt.gca().twinx()

        if do_reverse_cumsum:
            # We want a reverse cumsum, so reverse all values
            ys = ys[::-1]
            ys_good = ys_good[::-1]
            ys_bad = ys_bad[::-1]

        # Show how many traces we have seen so far
        cumsum = np.cumsum(ys)
        perc = [x * 100 / cumsum[-1] for x in cumsum]
        if do_reverse_cumsum:
            perc = perc[::-1]
        ax2.bar(xs, perc, **kwargs, color="black", alpha=0.3)

        # Make a red and a green bar for bad/good classification results
        cumsum_good = np.cumsum(ys_good)
        cumsum_bad = np.cumsum(ys_bad)
        perc_good = [x * 100 / (cumsum[-1]) for x in cumsum_good]
        perc_bad = [x * 100 / (cumsum[-1]) for x in cumsum_bad]
        if do_reverse_cumsum:
            perc_good = perc_good[::-1]
            perc_bad = perc_bad[::-1]

        # Instead of % of all traces, make it % of traces so far
        # If the first "bucket" is empty, then `p` can be 0 and this would be an invalid division
        # Therefore, guard against this. If the first bucket is empty, we do not want to plot anything here
        perc_good, perc_bad = (
            [x * 100 / p if p > 0 else 0 for x, p in zip(perc_good, perc)],
            [x * 100 / p if p > 0 else 0 for x, p in zip(perc_bad, perc)],
        )

        ax2.bar(xs, perc_bad, **kwargs, color="red", alpha=0.3)
        ax2.bar(xs, perc_good, **kwargs, bottom=perc_bad, color="green", alpha=0.3)
        ax2.set_ylim(0, 100)
        ax2.set_ylabel("% of Sequences")

        plt.title(
            f"Total Count of Sequences: {title}{' (Reverse Cumsum)' if do_reverse_cumsum else ''}"
        )
        plt.gcf().set_size_inches(15, 7.5)
        plt.tight_layout()
        plt.savefig(f"sequence-entropy-{title}.png")
        plt.show()
        plt.close()

# %%

