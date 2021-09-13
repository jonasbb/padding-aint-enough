#!/usr/bin/env python3

import csv
import shlex
import subprocess
import sys
import typing as t
from glob import glob
from os import path

FAILED_DOMAINS_LIST = (
    "/mnt/data/Downloads/new-task-setup/2018-10-01-no-dnssec/failed_domains_final.csv"
)
CONFUSION_DOMAINS_LISTS = [
    "/home/jbushart/projects/confusion_domains/redirects.csv",
    "/home/jbushart/projects/encrypted-dns/results/2018-10-09-no-dnssec/confusion_domains.csv",
]


class Canonicalize:
    cache: t.Dict[str, str]

    def __init__(self) -> None:
        self.cache = dict()
        # read all files and add them to the cache
        for file in CONFUSION_DOMAINS_LISTS:
            rdr = csv.reader(open(file))
            for dom, canon in rdr:
                dom = sys.intern(dom)
                canon = sys.intern(canon)
                # skip comments
                if dom.startswith("#"):
                    continue
                if dom in self.cache:
                    raise Exception(
                        f"Two duplicate entries for the same domain '{dom}' while canonicalizing."
                    )
                self.cache[dom] = canon

    def canonicalize(self, domain: str) -> str:
        res = domain
        try:
            res = self.cache[domain]
        except KeyError:
            pass
        return sys.intern(res)


def sanitize_file_name(filename: str) -> str:
    # strip extension
    tmp = path.basename(filename)
    if tmp.endswith(".xz"):
        tmp = tmp[: -len(".xz")]
    if tmp.endswith(".json"):
        tmp = tmp[: -len(".json")]
    if tmp.endswith(".dnstap"):
        tmp = tmp[: -len(".dnstap")]
    return sys.intern(tmp)


def load_files_to_ignore() -> t.Set[str]:
    res = set()
    rdr = csv.reader(open(FAILED_DOMAINS_LIST))
    # skip header
    next(rdr)
    for file, _reason in rdr:
        res.add(sanitize_file_name(file))
    return res


def get_label(filename: str, canonicalizer: Canonicalize) -> str:
    # get the name of the directory containing the file
    label = path.basename(path.dirname(filename))
    return canonicalizer.canonicalize(label)


def main() -> None:
    # Load a list of files to ignore
    # Load all the confusion domain information
    # Load all DNS requests

    canonicalizer = Canonicalize()
    files_to_ignore = load_files_to_ignore()

    # the dict key is the label, then a list of traces and each trace is a list of requests
    loaded_domains: t.Dict[str, t.List[t.List[str]]] = dict()

    for file in sorted(
        glob(
            "/mnt/data/Downloads/new-task-setup/2018-10-01-no-dnssec/processed/*/*.dnstap.*"
        )
    ):
        if sanitize_file_name(file) in files_to_ignore:
            continue
        label = get_label(file, canonicalizer)

        res = subprocess.run(
            [
                "fish",
                "-c",
                f"xzcat {shlex.quote(file)} | dnstap-ldns -r - | xsv search --delimiter ' ' --select 3 --no-headers 'FR' | xsv select 7-9",
            ],
            stdout=subprocess.PIPE,
            check=True,
        )
        dns_reqs = [
            sys.intern(req.replace(",", " "))
            for req in res.stdout.decode("utf-8").splitlines()
        ]
        loaded_domains.setdefault(label, []).append(dns_reqs)

    # per domain count in how many labels it appears and in how many traces
    # the first value in the tuple is per label, the second is per trace
    usage_per_domain: t.Dict[str, t.Tuple[t.Set[str], t.Set[str]]] = dict()
    for label, traces in loaded_domains.items():
        for trace_num, trace in enumerate(traces):
            for domain in trace:
                label_set, trace_set = usage_per_domain.setdefault(
                    domain, (set(), set())
                )
                label_set.add(label)
                trace_set.add(f"{label}-{trace_num}")

    counts_per_domain: t.Dict[str, t.Tuple[int, int]] = {
        domain: (len(labels), len(traces))
        for domain, (labels, traces) in usage_per_domain.items()
    }

    # for each trace count how many labels have this request
    traces_labelcount: t.List[t.List[int]] = [
        [counts_per_domain[domain][0] for trace in traces for domain in trace]
        for traces in loaded_domains.values()
    ]
    # for each trace count how many traces have this request
    traces_tracecount: t.List[t.List[int]] = [
        [counts_per_domain[domain][1] for trace in traces for domain in trace]
        for traces in loaded_domains.values()
    ]

    import IPython

    IPython.embed()


if __name__ == "__main__":
    main()
