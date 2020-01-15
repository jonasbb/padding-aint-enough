# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.3'
#       jupytext_version: 1.3.2
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %% [markdown]
# # Analyzing Variability in Encrypted DNS vs HTTP(s) vs Tor Traffic for Subpage-Agnostric Domain Fingerprinting

# %% [markdown]
# Measure the variability of:
#
# * Sequence Length
# * Packet count in / out / total
# * Bytes in / out / total
# * Direction changes
# * #Burst *== direction changes*
# * Burst Sizes

# %%
import dataclasses
import enum
import lzma
import typing as t
from collections import Counter
from enum import Enum
from glob import glob
from os import path

import matplotlib.pyplot as plt

import pylib
from scapy.all import *

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [15, 10]

# %%
basedir = "/mnt/data/Downloads/dnscapture-ndss2016-firefox-commoncrawl/"
basedir_tor = "/mnt/data/Downloads/dnscapture-ndss2016-tor-browser-commoncrawl/"

# %%
dnstap_seqs = pylib.load_folder(basedir, extension="dnstap")
dnstap_domain_2_feature = {
    domain: [seq.len() for seq in sequences] for domain, sequences in dnstap_seqs
}

# %%
pcap_seqs = pylib.load_folder(basedir, extension="pcap")
pcap_domain_2_feature = {
    domain: [seq.len() for seq in sequences] for domain, sequences in pcap_seqs
}

# %%
for data in [dnstap_domain_2_feature, pcap_domain_2_feature]:
    plt.violinplot(
        data.values(), vert=False, showmedians=True, showmeans=True, showextrema=False
    )
    labels = list(data.keys())
    plt.yticks(range(1, len(labels) + 1), labels)
    plt.xlim(left=0)
    plt.show()


# %%
class Direction(Enum):
    UPLOAD = enum.auto()
    DOWNLOAD = enum.auto()


# %%
from dataclasses_json import dataclass_json


# %%
@dataclass_json
@dataclasses.dataclass
class PcapFeatures:
    bytes_down: int = 0
    bytes_up: int = 0
    bytes_total: int = 0

    packets_down: int = 0
    packets_up: int = 0
    packets_total: int = 0

    current_direction: t.Optional[Direction] = None
    direction_changes: int = 0

    sequence_lengths: t.List[int] = dataclasses.field(default_factory=list)
    currenct_sequence_length: int = 0

    def new_packet(self, direction: Direction, payload_size: int) -> None:
        self._set_direction(direction)
        self._count_bytes(payload_size)

    def finish(self) -> None:
        self.current_direction = None
        if self.currenct_sequence_length > 0:
            self.sequence_lengths.append(self.currenct_sequence_length)
            self.currenct_sequence_length = 0

    def _set_direction(self, direction: Direction) -> None:
        if self.current_direction is not None and self.current_direction != direction:
            self.direction_changes += 1
            self.sequence_lengths.append(self.currenct_sequence_length)
            self.currenct_sequence_length = 0

        self.current_direction = direction
        self.currenct_sequence_length += 1

    def _count_bytes(self, count: int) -> None:
        assert self.current_direction is not None

        self.bytes_total += count
        self.packets_total += 1
        if self.current_direction == Direction.UPLOAD:
            self.bytes_up += count
            self.packets_up += 1
        else:
            self.bytes_down += count
            self.packets_down += 1


# %%
def process_packet(pkt: t.Any, features: PcapFeatures) -> None:
    # get TCP/UDP layer
    transport = pkt.payload.payload

    # skip empty packets or packets only containing TCP Flags
    if len(transport.payload) == 0:
        return
    
    # Skip DoT
    if transport.sport == 853 or transport.dport == 853:
        return

    direction = (
        Direction.UPLOAD if transport.dport in [80, 443, 853] else Direction.DOWNLOAD
    )
    features.new_packet(direction, len(transport.payload))


# %%
def process_pcap(file: str) -> PcapFeatures:
    pcap = rdpcap(lzma.open(file, "rb"))
    # Sort by time just in case they are unordered
    pcap = sorted(pcap, key=lambda pkt: pkt.time)
    features = PcapFeatures()
    for pkt in pcap:
        process_packet(pkt, features)
    features.finish()
    return features


# %%
from multiprocessing import Pool

# %%
pool = Pool()

for domain_folder in glob(path.join(basedir, "*")):
    domain = path.basename(domain_folder)
    print("Firefox", domain)
    pcap_features = pool.map(process_pcap, list(glob(path.join(domain_folder, "*.pcap.xz"))))
    json = PcapFeatures.schema().dumps(pcap_features, many=True)
    with open(f"pcap-{domain}-features.json", "wt") as f:
        f.write(json)

        
for domain_folder in glob(path.join(basedir_tor, "*")):
    domain = path.basename(domain_folder)
    print("Tor", domain)
    pcap_features = pool.map(process_pcap, list(glob(path.join(domain_folder, "*.pcap.xz"))))
    json = PcapFeatures.schema().dumps(pcap_features, many=True)
    with open(f"pcap-tor-{domain}-features.json", "wt") as f:
        f.write(json)
        
pool.join(10)

# %%
firefox_2_pcap_features = {}
for file in glob("pcap-*-features.json"):
    domain = file[len("pcap-"):-len("-features.json")]
    with open(file, "rt") as f:
        firefox_2_pcap_features[domain] = PcapFeatures.schema().loads(f.read(), many=True)

tor_2_pcap_features = {}
for file in glob("pcap-tor-*-features.json"):
    domain = file[len("pcap-tor-"):-len("-features.json")]
    with open(file, "rt") as f:
        tor_2_pcap_features[domain] = PcapFeatures.schema().loads(f.read(), many=True)

# %%
for field in dataclasses.fields(PcapFeatures):
    for browser, data in [("Firefox", firefox_2_pcap_features), ("Tor", tor_2_pcap_features)]:
        data = domain_2_pcap_features
        print(field.name)
        values = [[v.__dict__[field.name] for v in domain_values] for domain_values in data.values()]

        # Skip uninteresting cases
        if field.name in ["current_direction", "currenct_sequence_length"]:
            continue
        # Sequence lengths is an array, so we need to flatten one layer here
        if field.name == "sequence_lengths":
            new_values = list()
            for v1 in values:
                tmp = list()
                for v2 in v1:
                    tmp += v2
                length = len(tmp)
                tmp = filter(lambda x: x != 1, tmp)
                tmp = list(sorted(tmp)[:-int(length/50)])
                new_values.append(tmp)
            values = new_values

        plt.violinplot(
            values, vert=False, showmedians=True, showmeans=True, showextrema=False
        )
        labels = list(data.keys())
        plt.title(f"{browser} -- {field.name}")
        plt.yticks(range(1, len(labels) + 1), labels)
        plt.xlim(left=0)
        plt.show()

# %%
