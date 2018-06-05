#!/usr/bin/python3

import os
import pickle
import sys

import matplotlib.pyplot as plt
import numpy as np


def print_help(prg_name: str) -> None:
    print(f"""Usage
{prg_name} <File>

The program expects a single file as argument. The file must be a pickle-file containing the data used for plotting."""
          )


def main() -> None:
    if len(sys.argv) != 2:
        print_help(sys.argv[0])
        sys.exit(1)

    with open(sys.argv[1], "rb") as f:
        data = pickle.load(f)

    (root, _ext) = os.path.splitext(sys.argv[1])
    outfile = root + ".png"

    # Sort by time, such that the earliest start is at the top
    data.sort(
        key=
        lambda x: (x[2]["requestTime"] + x[2]["dnsStart"], x[2]["requestTime"] + x[2]["dnsEnd"]),
        reverse=True)

    begin = np.array([t["requestTime"] + t["dnsStart"] for (_, _, t) in data])
    end = np.array([t["requestTime"] + t["dnsEnd"] for (_, _, t) in data])
    event = [d for (d, _, _) in data]

    # for x in zip(begin, end, end-begin):
    #     print(x)

    plt.barh(range(len(begin)), end - begin, left=(begin - min(begin)))
    plt.yticks(range(len(begin)), event)
    fig = plt.gcf()
    fig.set_size_inches(min(max(max(end - begin), 5), 20), len(begin) / 3)
    # ensure there is enough space for the labels
    fig.tight_layout()
    fig.savefig(outfile)


if __name__ == "__main__":
    main()
