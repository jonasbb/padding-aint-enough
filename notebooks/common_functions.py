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

import pylib
import tabulate
from IPython.display import HTML, display


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
def autolabel(rects: t.Any, plt: t.Any) -> None:
    """
    Attach a text label above each bar displaying its height
    """
    for rect in rects:
        height = rect.get_height() + rect.get_y()
        plt.text(
            rect.get_x() + rect.get_width() / 2.0,
            height + 0.5,
            "%.1f" % round(height, 1),
            ha="center",
            va="bottom",
            rotation=0,
        )
