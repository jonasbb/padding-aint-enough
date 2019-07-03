# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 1.1.2
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
import pylib

# %%
import matplotlib.pyplot as plt

# %%
# %matplotlib inline
plt.rcParams['figure.figsize'] = [15, 10]

# %%
from glob import glob

# %%
base = "/mnt/data/Downloads/dnscaptures-countermeasures/extracted/jsons"

# %%
dnstaps = glob(base + "/**/*.dnstap.xz.json", recursive=True)

# %%
from os import path

# %%
dnstaps2pcap = {
    dnstap: dnstap.replace(".dnstap.xz.json", ".pcap.json") if path.exists(dnstap.replace(".dnstap.xz.json", ".pcap.json")) else None
    for dnstap in dnstaps
}

# %%
seqs = {
    **{
        f: pylib.load_file(f)
        for f in dnstaps2pcap.keys()
    },
    **{
        f: pylib.load_file(f)
        for f in dnstaps2pcap.values() if f is not None
    }
}

# %%
dnstap_dists = {
    dnstap: seqs[dnstap].distance(seqs[pcap])
    for dnstap, pcap in dnstaps2pcap.items() if pcap is not None
}

# %%
# split for pass and not-pass folders
pass_dists = []
ap_dists = []
for file, dist in dnstap_dists.items():
    if "_pass" in file:
        pass_dists.append(dist)
    else:
        ap_dists.append(dist)

# %%
len(pass_dists)

# %%
len(ap_dists)

# %%
plt.plot(sorted(pass_dists), label="pass")
plt.plot(sorted(ap_dists), label="ap")
plt.ylim(0, 5000)
plt.xlim(0, plt.xlim()[1])
plt.legend()

# %%


# %%


# %%


# %%


# %%
import scipy.stats

# %%
scipy.stats.describe(pass_dists)

# %%
scipy.stats.describe(ap_dists)

# %%
from functional import seq

# %%
seq(dnstap_dists.items()).filter(lambda x: "_pass" not in x[0] and x[1] < 100).map(lambda x: (*x, seqs[x[0]])).filter(lambda x: x[2].len() > 10)

# %%
s = list(seqs.values())[0]

# %%
s.len()

# %%
seq(dnstap_dists.items()).filter(lambda x: "_pass" in x[0] and x[1] > 1500).map(lambda x: (*x, seqs[x[0]], dnstaps2pcap[x[0]], seqs[dnstaps2pcap[x[0]]]))


# %% [markdown]
# # Calculate cross-distance stats
#
# The following variables exist:
#
# * Defence Scheme: pass/ap
# * Source: dnstap/pcap

# %%
def cross_distance(seqs):
    res = []
    for i, s in enumerate(seqs):
        for j, s2 in enumerate(seqs):
            if j > i:
                res.append(s.distance(s2))
    return res


# %%
cross_distance_stats = dict()
for defence, source in [("ap", "dnstap"),("ap", "pcap"), ("pass", "dnstap"),("pass", "pcap")]:
    stream = seq(dnstaps)
    
    if defence == "ap":
        stream = stream.filter(lambda x: "_pass" not in x)
    elif defence == "pass":
        stream = stream.filter(lambda x: "_pass" in x)
        
    if source == "pcap":
        stream = stream.map(lambda x: dnstaps2pcap[x]).filter(lambda x: x is not None)
        
    res = stream.sorted().group_by(lambda x: path.dirname(x)).map(lambda x: (x[0], cross_distance([seqs[f] for f in x[1]]))).map(lambda x: (path.basename(x[0]), (x[1], scipy.stats.describe(x[1])))).to_dict()
    cross_distance_stats[(defence, source)] = res

# %%
keys = cross_distance_stats.keys()
cross_distance_stats2 = dict()
for domain in cross_distance_stats[("pass", "dnstap")].keys():
    for key in keys:
        dists = cross_distance_stats[key].get(domain, None)
        if dists is not None:
            cross_distance_stats2.setdefault(domain, {})[key] = dists[1]

# %%
keys = cross_distance_stats.keys()
tmp = dict()
for k in keys:
    tmp[k] = []
for domain in cross_distance_stats[("pass", "dnstap")].keys():
    for key in keys:
        dists = cross_distance_stats[key].get(domain, None)
        if dists is not None:
            tmp[key].extend(dists[0])
for k in keys:
    print(k, scipy.stats.describe(tmp[k]))
    plt.plot(sorted(tmp[k]), label=str(k))
plt.legend()
plt.ylim(0, 3000)

# %%


# %%
dnstap_pass_lengths, dnstap_ap_lengths, pcap_pass_lengths, pcap_ap_lengths = [], [], [], []
for dnstap, pcap in dnstaps2pcap.items():
    dl, pl = None, None
    if "_pass" not in dnstap:
        dl, pl = dnstap_ap_lengths, pcap_ap_lengths
    else:
#     elif "_pass" in dnstap:
        dl, pl = dnstap_pass_lengths, pcap_pass_lengths
    
    dnstap = seqs[dnstap]
    dl.append(dnstap.len())
    if pcap is not None:
        pcap = seqs[pcap]
        pl.append(pcap.len())

# %%
plt.plot(sorted(dnstap_pass_lengths), label="dnstap-pass")
plt.plot(sorted(pcap_pass_lengths), label="pcap-pass")
plt.plot(sorted(dnstap_ap_lengths), label="dnstap-ap")
plt.plot(sorted(pcap_ap_lengths), label="pcap-ap")
plt.legend()

# %%
dnstaps2pcap.keys()


# %%
