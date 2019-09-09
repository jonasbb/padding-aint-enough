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
# # Clustering

# %% [markdown]
# We need 20 domains which are classified correctly and 10 domains which do not work.
# If a domain does not work, we also need to add the wrong label such that in the clustering they can mix.

# %%
import json
import lzma
import os.path as path
import typing as t
from collections import Counter
from glob import glob
from pprint import pprint

import matplotlib.pyplot as plt
import pylib
import scipy.cluster.hierarchy as cluster
from scipy.spatial.distance import pdist

# %%
file = "/home/jbushart/projects/encrypted-dns/results/2019-01-09-closed-world/misclassifications-final.json.xz"

# %%
data = [json.loads(line) for line in lzma.open(file)]

# %%
# Filter the data such that
# 1. Only k=1 is retained
# 2. The data is grouped by label
label_to_misclassifications: t.Dict[str, t.List[t.Any]] = dict()
for entry in data:
    if entry["k"] != 1:
        continue
    if entry["reason"] is not None:
        continue
    label_to_misclassifications.setdefault(entry["label"], list()).append(entry)

# %%
label_to_wrong_labels = {
    # Since this is k=1 there is only exactly 1 missclassification and
    # we do not actually need to properly process the `class_result`s
    label: Counter([entry["class_result"]["options"][0]["name"] for entry in entries])
    for label, entries in label_to_misclassifications.items()
}

# %% {"jupyter": {"outputs_hidden": true}}
for label, entries in label_to_misclassifications.items():
    if len(entries) == 10:
        print(label, len(entries))
        pprint(entries)

# %%
for label, counter in label_to_wrong_labels.items():
    if counter.most_common(1)[0][1] > 5:
        print(label, counter)

# %% [markdown]
# # List of 10 bad domains:
# * pages.tmall.com
# * content.tmall.com
# * panerabread.com
# * mericasads.com
# * caisse-epargne.fr
# * indianbank.net.in
# * reactjs.org
# * mega.co.nz
# * opencv.org
# * maxpreps.com

# %%
domains_bad = """
pages.tmall.com
content.tmall.com
panerabread.com
mericasads.com
caisse-epargne.fr
indianbank.net.in
reactjs.org
mega.co.nz
opencv.org
maxpreps.com
""".strip().splitlines()

# %%
# !cd /home/jbushart/projects/encrypted-dns/results/2019-01-09-closed-world; cat ./statistics-final.csv | xsv search --select=k 1 | xsv search --select=exact 10 | xsv select label | shuf --random-source=./statistics-final.csv | head -20

# %%
domains_good = """
kakao.com
mathrubhumi.com
constantcontact.com
crunchyroll.com
vietnamnet.vn
navyfederal.org
ibps.in
myanimesonline.net
playstation.com
tesla.com
ixl.com
oantagonista.com
carters.com
interia.pl
frys.com
bol.uol.com.br
sarayanews.com
forever21.tmall.com
scroll.in
animeyt.tv
""".strip().splitlines()

# %% [markdown]
# # Now that we have the good and bad domains, we can do the clustering

# %%
domains_all = domains_good + domains_bad

# %%
domains_good, domains_bad


# %% [markdown]
# ## Load all files

# %%
basefolder = "/mnt/data/Downloads/dnscaptures-main-group"

# %%
sequences_good = [
    pylib.load_file(file)
    for domain in domains_good
    for file in glob(path.join(basefolder, domain, "*-?-0.dnstap.xz"))
]
sequences_bad = [
    pylib.load_file(file)
    for domain in domains_bad
    for file in glob(path.join(basefolder, domain, "*-?-0.dnstap.xz"))
]

len(sequences_good), len(sequences_bad)

# %%
# pdist requires a 2-dimensional array, without any good reason
# So convert the list into an list of 1-element lists to fullfill this requirement
# The comparison lambda just has to take the 0th element every time
sequences_all = sequences_good + sequences_bad
sequences_matrix = [[s] for s in sequences_all]
distances_pairwise = pdist(
    sequences_matrix, lambda a, b: a[0].distance(b[0]) / max(a[0].len(), b[0].len())
)

# %%
z = cluster.linkage(distances_pairwise, method="single", optimal_ordering=True)


# %%
# %matplotlib inline

# %%
plt.rcParams["figure.figsize"] = [15, 60]

# %%
labels_all = [path.basename(s.id()).replace("-0.dnstap.xz", "") for s in sequences_all]

# %%
plt.figure()
dn = cluster.dendrogram(
    z,
    orientation="right",
    labels=labels_all,
    distance_sort="ascending",
    truncate_mode="none",
    color_threshold=9999,
    link_color_func=lambda x: "black",
)

label_on_index = dn["ivl"]

# https://python-graph-gallery.com/402-color-dendrogram-labels/
# Apply the right color to each label
my_palette = plt.cm.get_cmap("tab20", 30)
ax = plt.gca()
xlbls = ax.get_ymajorticklabels()
num = -1
for lbl in xlbls:
    num += 1
    val = domains_all.index(label_on_index[num][:-2])
    lbl.set_color(my_palette(val))

# plt.show()
plt.savefig("./clustering-k-1.svg")

# %%
