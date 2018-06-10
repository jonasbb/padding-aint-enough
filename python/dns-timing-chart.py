#!/usr/bin/python3

import os
import pickle
import sys
import typing as t

import IPython
import matplotlib.pyplot as plt
import numpy as np
from dateutil.parser import parse as parse_iso8601


def print_help(prg_name: str) -> None:
    print(f"""Usage
{prg_name} <File>

The program expects a single file as argument. The file must be a pickle-file containing the data used for plotting."""
          )


next_ind = -1


def get_next_ind() -> int:
    global next_ind
    tmp = next_ind
    next_ind -= 1
    return tmp


def source2color(source: str) -> str:
    if source == "Forwarder":
        return "r"
    elif source == "Client":
        return "g"
    else:
        return "b"


def source2height(source: str) -> float:
    if source == "Forwarder":
        return 0.66
    elif source == "Client":
        return 1
    else:
        return 0.33


def main() -> None:
    if len(sys.argv) != 2:
        print_help(sys.argv[0])
        sys.exit(1)

    with open(sys.argv[1], "rb") as f:
        data = pickle.load(f)

    (root, _ext) = os.path.splitext(sys.argv[1])
    outfile = root + ".svg"

    # Sort by time, such that the earliest start is at the top
    data.sort(
        key=
        lambda x: (x[2]["requestTime"] + x[2]["dnsStart"], x[2]["requestTime"] + x[2]["dnsEnd"]),
        reverse=True)

    begin = np.array([t["requestTime"] + t["dnsStart"] for (_, _, t) in data])
    end = np.array([t["requestTime"] + t["dnsEnd"] for (_, _, t) in data])
    event = [
        f"{d} ({round(t * 1000, 3)} ms)"
        for ((d, _, _), t) in zip(data, end - begin)
    ]

    # plot first part
    plt.barh(range(len(begin)), end - begin, left=(begin - min(begin)))
    # plt.yticks(range(len(begin)), event)
    yticks = list(range(len(begin)))
    yticks_labels = list(event)

    # also consume DNS information if available
    dns_pickle = os.path.join(os.path.dirname(sys.argv[1]), "dns.pickle")
    if os.path.exists(dns_pickle):
        dns = pickle.load(open(dns_pickle, 'rb'))
        dns.sort(key=lambda x: x['start'], reverse=False)

        dns_start = np.array(
            [parse_iso8601(elem['start']).timestamp() for elem in dns])
        min_dns_start = min(dns_start)
        dns_end = np.array(
            [parse_iso8601(elem['end']).timestamp() for elem in dns])
        dns_names = np.array(
            [f"{elem['qname']} ({elem['qtype']})" for elem in dns])
        dns_source = np.array([elem['source'][0] for elem in dns])

        label2index: t.Dict[str, int] = dict()

        for (source, label, start, end) in zip(dns_source, dns_names,
                                               dns_start, dns_end):
            if label not in label2index.keys():
                label2index[label] = get_next_ind()
            ind = label2index[label]
            plt.barh(
                ind,
                end - start,
                left=start - min_dns_start,
                color=source2color(source),
                alpha=0.5,
                height=source2height(source))

        for (label, ind) in label2index.items():
            yticks.append(ind)
            yticks_labels.append(label)

    plt.yticks(yticks, yticks_labels)
    fig = plt.gcf()
    fig.set_size_inches(15, len(yticks) / 3 + 0.6)
    # ensure there is enough space for the labels
    fig.tight_layout()
    fig.savefig(outfile)


if __name__ == "__main__":
    main()
