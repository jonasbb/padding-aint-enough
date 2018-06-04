#!/usr/bin/python3

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

    begin = np.array([t["requestTime"] + t["dnsStart"] for (_, _, t) in data])
    end = np.array([t["requestTime"] + t["dnsEnd"] for (_, _, t) in data])
    event = [d for (d, _, _) in data]

    # for x in zip(begin, end, end-begin):
    #     print(x)

    plt.barh(range(len(begin)), end - begin, left=begin)
    plt.yticks(range(len(begin)), event)
    plt.show()


if __name__ == "__main__":
    main()
