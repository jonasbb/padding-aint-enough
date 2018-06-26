#!/usr/bin/python3

import os
import pickle
import sys
import typing as t

import IPython
import matplotlib.pyplot as plt
import numpy as np
from dateutil.parser import parse as parse_iso8601
from matplotlib.lines import Line2D


def print_help(prg_name: str) -> None:
    print(
        f"""Usage
{prg_name} <File>

The program expects a single file as argument. The file must be a pickle-file containing the data used for plotting."""
    )


next_ind = -1


def get_next_ind() -> int:
    global next_ind
    tmp = next_ind
    next_ind -= 1
    return tmp


def mk_legend() -> t.List[Line2D]:
    return [
        Line2D([0], [0], color="r", lw=4, label="1 Pkt"),
        Line2D([0], [0], color="y", lw=4, label="2 Pkts"),
        Line2D([0], [0], color="m", lw=4, label="3 Pkts"),
        Line2D([0], [0], color="b", lw=4, label="4 Pkts"),
        Line2D([0], [0], color="k", lw=4, label="5+ Pkts"),
    ]


def info_from_source(source: str, size: int) -> t.Tuple[str, float, float]:
    height = None
    color = None
    alpha = None
    if source == "Forwarder":
        height = 1.
        alpha = 0.66
        if size <= 1 * 468:
            # red
            color = "r"
        elif size <= 2 * 468:
            # yellow
            color = "y"
        elif size <= 3 * 468:
            # magenta
            color = "m"
        elif size <= 4 * 468:
            # blue
            color = "b"
        else:
            # black
            color = "k"
    elif source == "Client":
        height = 0.5
        color = "g"
        alpha = 0.33
    else:
        height = 0.33
        color = "crimson"
        alpha = 0.5

    return (color, height, alpha)


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
        key=lambda x: (
            x[2]["requestTime"] + x[2]["dnsStart"],
            x[2]["requestTime"] + x[2]["dnsEnd"],
        ),
        reverse=True,
    )

    begin = np.array([t["requestTime"] + t["dnsStart"] for (_, _, t) in data])
    end = np.array([t["requestTime"] + t["dnsEnd"] for (_, _, t) in data])
    event = [
        f"{d} ({round(t * 1000, 3)} ms)" for ((d, _, _), t) in zip(data, end - begin)
    ]

    if len(data) > 0:
        minimum_size = (max(end) - min(begin)) * 0.01
    else:
        minimum_size = 0

    # also consume DNS information if available
    dns_pickle = os.path.join(os.path.dirname(sys.argv[1]), "dns.pickle")
    if os.path.exists(dns_pickle):
        dns = pickle.load(open(dns_pickle, "rb"))
        dns.sort(key=lambda x: x["start"], reverse=False)

        dns_start = np.array([parse_iso8601(elem["start"]).timestamp() for elem in dns])
        min_dns_start = min(dns_start)
        dns_end = np.array([parse_iso8601(elem["end"]).timestamp() for elem in dns])
        dns_names = np.array([f"{elem['qname']} ({elem['qtype']})" for elem in dns])
        dns_source = np.array([elem["source"][0] for elem in dns])
        dns_size = np.array([elem["response_size"] for elem in dns])

        minimum_size = max(minimum_size, (max(dns_end) - min_dns_start) * 0.01)

    ensure_size = lambda x: max(x, minimum_size)

    if len(data) > 0:
        # plot Chrome's reported DNS times
        # The real time is with 100% color, the extended size is with lower alpha
        plt.barh(
            range(len(begin)),
            end - begin,
            left=(begin - min(begin)),
            color="dodgerblue",
            alpha=1,
        )
        plt.barh(
            range(len(begin)),
            [ensure_size(x) for x in (end - begin)],
            left=(begin - min(begin)),
            alpha=0.5,
            color="dodgerblue",
        )
    # plt.yticks(range(len(begin)), event)
    yticks = list(range(len(begin)))
    yticks_labels = list(event)

    if os.path.exists(dns_pickle):
        label2index: t.Dict[str, int] = dict()
        prev_end = None

        for (source, label, start, end, response_size) in zip(
            dns_source, dns_names, dns_start, dns_end, dns_size
        ):
            if label not in label2index.keys():
                label2index[label] = get_next_ind()
            ind = label2index[label]
            (color, height, alpha) = info_from_source(source, response_size)

            if source == "ForwarderLostQuery":
                ((x1, y1), (x2, y2)) = (
                    (start - min_dns_start, ind - 0.5),
                    (start - min_dns_start, ind + 0.5),
                )
                plt.plot((x1, x2), (y1, y2), "k-", linewidth=.5, snap=True)
            else:
                width = end - start
                # if the plot would be too thin, create a wider one but with less alpha
                if ensure_size(width) > width:
                    plt.barh(
                        ind,
                        ensure_size(end - start),
                        left=start - min_dns_start,
                        color=color,
                        alpha=alpha / 3,
                        height=height,
                    )

                plt.barh(
                    ind,
                    end - start,
                    left=start - min_dns_start,
                    color=color,
                    alpha=alpha,
                    height=height,
                )

                # put text labels next to every Forwarder time range with information about
                # 1. the time difference to the previous message
                # 2. the duration the request itself took
                if source == "Forwarder":
                    label = ""
                    if prev_end:
                        label += f" 𝚫 {round((start-prev_end) * 1000, 3)} ms "
                    prev_end = end
                    label += f"\n⏱ {round((end-start) * 1000, 3)} ms"
                    plt.text(
                        end - min_dns_start,
                        ind,
                        label,
                        horizontalalignment="left",
                        verticalalignment="center",
                        fontname="Symbola",
                    )

        for (label, ind) in label2index.items():
            yticks.append(ind)
            yticks_labels.append(label)

    plt.yticks(yticks, yticks_labels)
    plt.xlabel("Time in seconds")
    fig = plt.gcf()
    lgd = fig.legend(
        handles=mk_legend(),
        loc="upper left",
        bbox_to_anchor=(1, 0.95),
        bbox_transform=fig.transFigure,
        fancybox=True,
    )
    fig.set_size_inches(15, len(yticks) / 2.5 + 0.6)
    # ensure there is enough space for the labels
    fig.tight_layout()
    fig.savefig(outfile, bbox_extra_artists=(lgd,), bbox_inches="tight")


if __name__ == "__main__":
    main()
