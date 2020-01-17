# -*- coding: utf-8 -*-
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

# %% [markdown]
# How to measure the variability?
# The goal is to reduce the violin plot into a single number, which is small if the variability is low.
#
# * Entropy
#     * What kind of entropy measure works well on a list of integers?
# * Interquartile range or 5%/95% values
#     * Needs some normalization to be comparable between different absolute values.
#     * For a split distribution (2 heaps), this might be higher, but the distribution might only contain two distinct values.

# %% [markdown]
# ### Potential Data Problems
#
# * Firefox DNS Dataset
#     * Extracting the bytes or packets from the pcap will yield wrong results.
#         The results are worse for for bytes though.
#         The main reason are the `start.example.`/`end.example.` and the two large `aa*`/`zz*` queries.
#
#         The overhead in bytes is roughly:
#         * `aa*`/`zz*`
#             415/415 up
#             1295/1301 down
#         * `start.example.`/`end.example.`
#             159/159 up
#             1068/1066 down

# %% [markdown]
# # FIXMEs
#
# * Fix typo in `currenct_sequence_length`

# %%
# pylint: disable=c-extension-no-member
# pylint: disable=redefined-outer-name
# %%
import dataclasses
import enum
import os
import typing as t
from glob import glob
from multiprocessing import Pool
from os import path

import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pylib
import scipy
import tabulate
from dataclasses_json import dataclass_json
from IPython.display import HTML, display
from scapy.all import rdpcap

# %%
# %matplotlib inline
plt.rcParams["figure.figsize"] = [15, 10]

# %%
basedir = "/mnt/data/Downloads/dnscapture-ndss2016-firefox-commoncrawl/"
basedir_tor = "/mnt/data/Downloads/dnscapture-ndss2016-tor-browser-commoncrawl/"


@dataclasses.dataclass
class Configuration:
    browser: str
    basedir: str
    prefix: str
    extractor: t.Callable[[str], "PcapFeatures"]


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


# %% [markdown]
# #### Types and functions to process PCAPs and Packets

# %%
class Direction(enum.Enum):
    UPLOAD = enum.auto()
    DOWNLOAD = enum.auto()


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


# Supress pylint errors
PcapFeatures.schema = PcapFeatures.schema  # type: ignore


# %%
def process_packet(
    pkt: t.Any, features: PcapFeatures, filter_ports: t.List[int]
) -> None:
    # get TCP/UDP layer
    transport = pkt.payload.payload

    # skip empty packets or packets only containing TCP Flags
    if len(transport.payload) == 0:
        return

    # Skip traffic we are not looking for
    if transport.sport not in filter_ports and transport.dport not in filter_ports:
        return

    direction = (
        Direction.UPLOAD if transport.dport in filter_ports else Direction.DOWNLOAD
    )
    features.new_packet(direction, len(transport.payload))


# %%
def process_pcap(file: str, filter_ports: t.List[int]) -> PcapFeatures:
    pcap = rdpcap(open(file, "rb"))
    # Sort by time just in case they are unordered
    pcap = sorted(pcap, key=lambda pkt: pkt.time)
    features = PcapFeatures()
    for pkt in pcap:
        process_packet(pkt, features, filter_ports)
    features.finish()
    return features


def process_pcap_dns(file: str) -> PcapFeatures:
    return process_pcap(file, [53, 853])


def process_pcap_http_tor(file: str) -> PcapFeatures:
    return process_pcap(file, [80, 443])


# %%
configurations = {
    "dns": Configuration(
        browser="Firefox DNS",
        basedir=basedir,
        prefix="pcap-dns-",
        extractor=process_pcap_dns,
    ),
    "firefox": Configuration(
        browser="Firefox",
        basedir=basedir,
        prefix="pcap-firefox-",
        extractor=process_pcap_http_tor,
    ),
    "tor": Configuration(
        browser="Tor Browser",
        basedir=basedir_tor,
        prefix="pcap-tor-",
        extractor=process_pcap_http_tor,
    ),
}

# %%
# Pool initialization has to be deferred until all functions which are later used by the pool are defined
pool = Pool()

# %%
# Process the PCAP files and save the extracted features as JSON

for config in configurations.values():
    for domain_folder in glob(path.join(config.basedir, "*")):
        domain = path.basename(domain_folder)
        print(config.browser, domain)
        # mypy wrongly thinks that the function has a self argument
        # https://github.com/python/mypy/issues/5485
        extractor: t.Callable[[str], PcapFeatures] = config.extractor  # type: ignore
        pcap_features = pool.map(
            extractor, list(glob(path.join(domain_folder, "*.pcap")))
        )
        # mypy doesn't understand the schema()
        json = PcapFeatures.schema().dumps(pcap_features, many=True)  # type: ignore
        with open(f"{config.prefix}{domain}-features.json", "wt") as f:
            f.write(json)


# %%
# Load above JSON and make it usable for the rest of the program


def load_features_from_json(config: Configuration) -> t.Dict[str, t.List[PcapFeatures]]:
    features = {}
    for file in glob(f"{config.prefix}*-features.json"):
        domain = file[len(config.prefix) : -len("-features.json")]
        with open(file, "rt") as f:
            # mypy doesn't understand the schema()
            features[domain] = PcapFeatures.schema().loads(  # type: ignore
                f.read(), many=True
            )
    return features


dns_2_pcap_features = load_features_from_json(configurations["dns"])
firefox_2_pcap_features = load_features_from_json(configurations["firefox"])
tor_2_pcap_features = load_features_from_json(configurations["tor"])


# %%
def measure_uniformity_5pc_norm(values: t.List[t.List[int]]) -> t.Tuple[float, float]:
    """Normalized 95%-5% range"""

    def measure_domain_uniformity(domain_values: t.List[int]) -> float:
        r = 5
        med = np.median(domain_values)
        lower = np.percentile(domain_values, r)
        upper = np.percentile(domain_values, 100 - r)
        return (upper - lower) / med

    tmp = [measure_domain_uniformity(dv) for dv in values]
    return np.median(tmp), np.std(tmp)


def measure_uniformity_20pc_norm(values: t.List[t.List[int]]) -> t.Tuple[float, float]:
    """Normalized 80%-20% range"""

    def measure_domain_uniformity(domain_values: t.List[int]) -> float:
        r = 20
        med = np.median(domain_values)
        lower = np.percentile(domain_values, r)
        upper = np.percentile(domain_values, 100 - r)
        return (upper - lower) / med

    tmp = [measure_domain_uniformity(dv) for dv in values]
    return np.median(tmp), np.std(tmp)


def measure_uniformity_5pc(values: t.List[t.List[int]]) -> t.Tuple[float, float]:
    """95%-5% range"""

    def measure_domain_uniformity(domain_values: t.List[int]) -> float:
        r = 5
        lower = np.percentile(domain_values, r)
        upper = np.percentile(domain_values, 100 - r)
        return upper - lower

    tmp = [measure_domain_uniformity(dv) for dv in values]
    return np.median(tmp), np.std(tmp)


def measure_uniformity_20pc(values: t.List[t.List[int]]) -> t.Tuple[float, float]:
    """80%-20% range"""

    def measure_domain_uniformity(domain_values: t.List[int]) -> float:
        r = 20
        lower = np.percentile(domain_values, r)
        upper = np.percentile(domain_values, 100 - r)
        return upper - lower

    tmp = [measure_domain_uniformity(dv) for dv in values]
    return np.median(tmp), np.std(tmp)


def entropy(labels: t.List[int], base: t.Optional[int] = None) -> float:
    _value, counts = np.unique(labels, return_counts=True)
    return scipy.stats.entropy(counts, base=base)


def measure_uniformity_entropy(values: t.List[t.List[int]]) -> t.Tuple[float, float]:
    """Entropy base-2"""

    tmp = [entropy(dv, base=2) for dv in values]
    return np.median(tmp), np.std(tmp)


# %%
# measure type -> feature -> browser -> t.Tuple[float, float]
results: t.Dict[str, t.Dict[str, t.Dict[str, t.Tuple[float, float]]]] = {}

for measure_uniformity in [
    measure_uniformity_5pc_norm,
    measure_uniformity_5pc,
    measure_uniformity_20pc_norm,
    measure_uniformity_20pc,
    measure_uniformity_entropy,
]:
    measure = measure_uniformity.__doc__
    if measure is None:
        measure = ""
    measure.strip()
    display(HTML(f"<h1>{measure}</h1>"))
    for field in dataclasses.fields(PcapFeatures):
        # https://stackoverflow.com/a/58324984
        legends = []

        def add_legend(violin: t.Any, label: str) -> None:
            color = violin["bodies"][0].get_facecolor().flatten()
            # legends is defined outside of this function on purpose
            legends.append(  # pylint: disable=cell-var-from-loop
                (mpatches.Patch(color=color), label)
            )

        text = measure + "\n\n"

        for config, data in [
            (configurations["dns"], dns_2_pcap_features),
            (configurations["firefox"], firefox_2_pcap_features),
            (configurations["tor"], tor_2_pcap_features),
        ]:
            values = [
                [v.__dict__[field.name] for v in domain_values]
                for domain_values in data.values()
            ]

            # Skip uninteresting cases
            if field.name in ["current_direction", "currenct_sequence_length"]:
                continue
            # Sequence lengths is an array, so we need to flatten one layer here
            if field.name == "sequence_lengths":
                new_values = list()
                for v1 in values:
                    tmp: t.List[int] = list()
                    for v2 in v1:
                        tmp += v2
                    length = len(tmp)
                    #                 tmp = list(filter(lambda x: x != 1, tmp))
                    tmp = sorted(tmp)[: -int(length / 50)]
                    new_values.append(tmp)
                values = new_values

            uniformity, std_dev = measure_uniformity(values)
            text += f"{config.browser:>11}: {uniformity:3.02f} ± {std_dev:3.02f}\n"
            results.setdefault(measure, {}).setdefault(field.name, {})[
                config.browser
            ] = (uniformity, std_dev)

            add_legend(
                plt.violinplot(
                    values,
                    vert=False,
                    showmedians=True,
                    showmeans=True,
                    showextrema=False,
                ),
                config.browser,
            )
            labels = list(data.keys())
            plt.yticks(range(1, len(labels) + 1), labels)
        #         plt.xlim(left=0)
        #         plt.title(f"{config.browser} -- {field.name}")
        #         plt.show()

        if len(legends) > 0:
            trans = plt.axes().transAxes
            plt.text(
                0.95,
                0.05,
                text.strip(),
                transform=trans,
                fontsize=14,
                horizontalalignment="right",
                verticalalignment="bottom",
            )
            plt.xlim(left=0)
            plt.legend(*zip(*legends), loc="upper right")
            plt.title(f"Combined -- {field.name}")
            plt.tight_layout()
            filepath = f"figs/{measure}/{field.name}.svg"
            os.makedirs(path.dirname(filepath), exist_ok=True)
            plt.savefig(filepath)
            plt.show()
    display(HTML(f"<hr />"))

for measure, measure_values in results.items():
    # get table header
    keys = list(next(iter(measure_values.values())).keys())
    table = [[""] + keys]
    for feature, feature_values in measure_values.items():
        table.append(
            [feature]
            + [f"{a:2.2f} ± {b:2.2f}" for a, b in [feature_values[k] for k in keys]]
        )
    display(HTML(f"<h1>{measure}</h1>"))
    display(HTML(tabulate.tabulate(table, tablefmt="html")))
    display(HTML(f"<hr />"))

# %%
