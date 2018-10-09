import typing as t
from collections import OrderedDict
from copy import copy
from itertools import cycle
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# We need to reset the state of matplotlib between executions of this script
plt.cla()
plt.clf()

# Make sure the config dict always exists
if "config" not in dir():
    config: t.Dict[str, t.Any] = dict()

# Provide type annotations for all variables passed into this script
if False:  # pylint: disable=W0125
    # array of (label, value) pair
    rawdata: t.List[t.Tuple[str, t.List[float]]] = list()
    rawimgpath: str = ""

if "__file__" in globals():
    # being run as freestanding script
    import sys
    import pickle

    rawdata, config = pickle.load(open(sys.argv[1], "rb"))
    rawimgpath = sys.argv[1] + ".svg"

imgpath = Path(rawimgpath)

# convert to dict but keep order
plotdata = OrderedDict(rawdata)
for value in plotdata.values():
    value.append(1)
# Make data
data = pd.DataFrame(plotdata)

# The following list comprehension requires that the variable data_perc is in the global namespace
global data_perc  # pylint: disable=W0604
# We need to transform the data from raw data to percentage (fraction)
data_perc = data.divide(data.sum(axis=1), axis=0)

# # Make the plot
# # This plot uses filled lines, where each entry is only a single point, thus the curves are
# # always diagonal
# kwargs = dict()
# if "colors" in config:
#     kwargs["colors"] = config["colors"]
# plt.gca().stackplot(
#     range(1, len(next(iter(plotdata.values()))) + 1),
#     *[data_perc[label] for label in plotdata.keys()],
#     labels=list(plotdata.keys()),
#     **kwargs
# )

# Make the plot
# This plot simulates a stacked bar plot, but is more efficient to draw
kwargs = dict()
if "colors" in config:
    colors = cycle(config["colors"])
else:
    colors = cycle([None])
size = len(next(iter(plotdata.values())))
line = np.zeros(size)
# for (label, color) in reversed(list(zip(plotdata.keys(), colors))):
for (label, color) in zip(plotdata.keys(), colors):
    if color:
        kwargs["color"] = color

    before = copy(line)
    line += data_perc[label]
    plt.fill_between(
        range(1, size + 1),
        line,
        before,
        step="post",
        label=label,
        linewidth=0,
        **kwargs
    )

fig = plt.gcf()
fig.set_size_inches(12, 6)
# remove any margins around the figure
plt.margins(0, 0)
# put the upper left corner of the legend at coordinates (1, 1)
plt.gca().legend(loc="upper left", bbox_to_anchor=(1, 1), borderaxespad=1)
plt.title("100% stacked area chart")

if "xticks" in config:
    plt.xticks([x + 0.5 for x in range(1, size)], config["xticks"], rotation="vertical")

# support being called with a filename without any extension
# In order to really use the filename (and not append an extension), we need to specify
# a format
kwargs = dict()
if imgpath.suffix == "":
    kwargs["fileformat"] = "png"
plt.savefig(imgpath, bbox_inches="tight", **kwargs)
