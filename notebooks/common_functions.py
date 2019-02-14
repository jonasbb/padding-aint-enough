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

# %% [markdown]
# # About
#
# This notebook contains Python functions common between multiple notebooks.
# Thanks to jupytext, this notebook can be imported as `import common_functions` or `from common_functions import *`.

# %%
import os.path
import typing as t
from glob import glob
from itertools import cycle

import matplotlib.cm
import pylib
import tabulate
from IPython.display import HTML, display

# %%
# Use real proper labels
LABELS = ["Pseudo-Plurality", "Plurality", "Majority", "Unanimous"]
COLORS = cycle(matplotlib.cm.Set1.colors)  # pylint: disable=E1101
COLORS2 = cycle(matplotlib.cm.tab10.colors)  # pylint: disable=E1101
HATCHES = cycle(["/", "-", "\\", "|"])

# %%
def show_infos_for_domain(domain: str) -> None:
    basefolder = (
        "/mnt/data/Downloads/dnscaptures-large-again/dnscaptures/working/processed"
    )

    table = [
        [repr(seq), seq.classify()]
        for seq in map(
            pylib.load_file, glob(os.path.join(basefolder, domain, "*-*-0.dnstap*"))
        )
    ]

    display(HTML(tabulate.tabulate(table, tablefmt="html")))


# %%
def autolabel(
    rects: t.Any,
    plt: t.Any,
    *,
    yoffset: t.Union[None, float, t.List[float]] = None,
    precision: int = 1,
) -> None:
    """
    Attach a text label above each bar displaying its height

    yoffset: Allows moving the text on the y-axis by a fixed amount
    The value must be a scalar, applying the same offset to all bars, or a list with one entry per bar
    """
    yoffsets: t.List = []

    if yoffset is None:
        yoffsets = [0] * len(rects)
    elif isinstance(yoffset, float):
        yoffsets = [yoffset] * len(rects)
    elif isinstance(yoffset, list):
        assert len(rects) == len(yoffset)
        yoffsets = yoffset
    else:
        raise Exception(
            "Unkown type for `yoffsets`, needs to be scalar or list of scalar."
        )

    for rect, offset in zip(rects, yoffsets):
        height = rect.get_height() + rect.get_y()
        plt.text(
            rect.get_x() + rect.get_width() / 2.0,
            height + 0.5 + offset,
            f"{height:.{precision}f}",
            ha="center",
            va="bottom",
            rotation=0,
        )


# %%
def label2good_label(label: str) -> str:
    if label == "Exact":
        return "Unanimous"
    if label == "PluralityThenMinDist":
        return "Pseudo-Plurality"
    return label
