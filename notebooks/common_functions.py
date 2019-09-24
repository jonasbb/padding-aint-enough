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
# # About
#
# This notebook contains Python functions common between multiple notebooks.
# Thanks to jupytext, this notebook can be imported as `import common_functions` or `from common_functions import *`.

# %%
import lzma
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
def open_file(path: str, mode: str = "rt") -> t.Any:
    """
    Open files, also compressed, transparently

    Open compressed files like a normal file.
    """
    file = None
    ext = os.path.splitext(path)[1]
    if ext == ".xz":
        return lzma.open(path, mode)

    return open(path, mode)
