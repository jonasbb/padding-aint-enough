#!/usr/bin/env python3

import csv
import pickle
import sys
import typing as t
from pathlib import Path

from tldextract import extract

# Type definitions
KeyType = t.Tuple[int, str]
CountsType = t.List[int]
AggType = t.Dict[KeyType, CountsType]
ResType = t.List[t.Tuple[KeyType, CountsType]]


def main(file: Path) -> None:
    reader = csv.DictReader(file.open())
    # skip header row
    next(reader)

    aggregate: AggType = dict()
    for row in reader:
        key = (int(row["k"]), extract(row["label"]).suffix)
        res = aggregate.get(key, [0, 0, 0, 0, 0, 0, 0])
        res[0] += int(row["corr"])
        res[1] += int(row["corr_w_reason"])
        res[2] += int(row["und"])
        res[3] += int(row["und_w_reason"])
        res[4] += int(row["wrong"])
        res[5] += int(row["wrong_w_reason"])
        res[6] += 10
        aggregate[key] = res

    # The shape is as follows
    # {
    #     (k, TLD): [corr, corr_w_reason, und, und_w_reason, wrong, wrong_w_reason, total],
    #
    #     # types:
    #     (int, str): [int, int, int, int, int, int, int],
    #     ...
    # }

    # convert to list
    results: ResType = list(aggregate.items())
    # sort by TLD
    results.sort(key=lambda x: tuple(reversed(x[0][1].split("."))))
    # filter TLDs with many domains
    results = [x for x in results if x[1][6] > 300]

    # The shape is as follows
    # [
    #     ((k, TLD), [corr, corr_w_reason, und, und_w_reason, wrong, wrong_w_reason, total]),
    #     ...
    # ]

    # partition by k
    partitions: t.Dict[int, ResType] = dict()
    for r in results:
        partitions.setdefault(r[0][0], list()).append(r)

    for (k, data) in partitions.items():
        to_pickle = (
            [
                ("Correct", [x[1][0] + 0.1 for x in data]),
                ("Correct (wR)", [x[1][1] + 0.1 for x in data]),
                ("Undetermined", [x[1][2] + 0.1 for x in data]),
                ("Undetermined (wR)", [x[1][3] + 0.1 for x in data]),
                ("Wrong", [x[1][4] + 0.1 for x in data]),
                ("Wrong (wR)", [x[1][5] + 0.1 for x in data]),
            ],
            {
                "colors": [
                    "#349e35",
                    "#98dd8b",
                    "#df802e",
                    "#feba7c",
                    "#d33134",
                    "#fe9897",
                ],
                "xticks": [x[0][1] for x in data],
            },
        )
        pickle.dump(to_pickle, file.parent.joinpath(f"per-tld.k{k}.pickle").open("wb"))


if __name__ == "__main__":
    for value in sys.argv[1:]:
        main(Path(value))
