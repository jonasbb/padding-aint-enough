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
# # Keras Hyperparameter Analysis
#
# This script analyses the different Keras runs and shows which hyperparameters work better than others.
# This is usefull to get a more intuitive understand of what works after using Talos to test a large matrix.

# %%
import csv
import json
import os.path
import typing as t
from glob import glob

import matplotlib.pyplot as plt
from natsort import natsorted

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [15, 10]

# %%
files = glob("../keras/csvs/*.csv")

# %%
def fix_value(value: str) -> str:
    return value.replace("NA", "nan")


def load_file(file: str) -> t.Tuple[t.Dict[str, float], t.Dict[str, float]]:
    content = dict()
    with open(file) as csvfile:
        reader = csv.DictReader(csvfile)
        rows = list(reader)
        for key in rows[0].keys():
            if "loss" in key:
                content[key] = min(float(fix_value(row[key])) for row in rows)
            else:
                content[key] = max(float(fix_value(row[key])) for row in rows)

    file = os.path.basename(file)
    timestamp = file.split("{")[0]
    # cut off timestamp and `.csv` at the end
    j = file[len(timestamp) : -4]
    # make it valid JSON
    j = (
        j.replace("'", '"')
        .replace("None", "null")
        .replace("<class ", "")
        .replace(">", "")
    )
    meta = json.loads(j)
    meta["timestamp"] = timestamp

    try:
        meta["optimizer"] = meta["optimizer"].split(".")[-1]
    except KeyError:
        pass

    return (meta, content)


# %%
def filter_data(tmp: t.Tuple[t.Dict[str, t.Any], t.Dict[str, float]]) -> bool:
    (meta, _content) = tmp
    # These three activation functions produce terrible results, so ignore them
    if meta["activation"] in ["elu", "linear", "relu"]:
        return False

    # default
    return True


# %%
data = [tmp for tmp in (load_file(f) for f in files) if filter_data(tmp)]

# %%
metric = "accuracy"

# %%
for variable in data[0][0].keys():
    if variable == "timestamp":
        continue
    values: t.Dict[t.Any, t.List[float]] = dict()
    for meta, content in data:
        values.setdefault(meta[variable], list()).append(content[metric])
    try:
        values["None"] = values[None]
        del values[None]
    except KeyError:
        pass

    labels = natsorted(list(values.keys()))
    ys = [values[l] for l in labels]
    labels = [f"{l} ({len(y)})" for l, y in zip(labels, ys)]

    plt.boxplot(ys, labels=labels)
    plt.title(f"Free Variable: {variable}")
    plt.ylabel(f"Metric: {metric}")
    plt.ylim(bottom=0, top=1)
    plt.show()

# %%
