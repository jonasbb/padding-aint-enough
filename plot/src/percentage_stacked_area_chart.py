# library
import pickle
import sys
from collections import OrderedDict

import matplotlib.pyplot as plt
import pandas as pd

# array of (label, value) pair
plotdata = pickle.load(open(sys.argv[1], "rb"))
# convert to dict but keep order
plotdata = OrderedDict(plotdata)
# Make data
data = pd.DataFrame(plotdata)

# We need to transform the data from raw data to percentage (fraction)
data_perc = data.divide(data.sum(axis=1), axis=0)

# Make the plot
plt.stackplot(
    range(1, len(next(iter(plotdata.values()))) + 1),
    *[data_perc[label] for label in plotdata.keys()],
    labels=list(plotdata.keys()),
)
plt.legend(loc="upper left")
plt.margins(0, 0)
plt.title("100 % stacked area chart")
plt.savefig(sys.argv[2], bbox_inches="tight", format="png")
