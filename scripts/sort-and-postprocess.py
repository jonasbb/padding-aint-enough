#!/usr/bin/env python3

import lzma
import multiprocessing as mp
import os
import re
import subprocess
import sys
import typing as t
from glob import glob
from os import path
from shutil import copy2 as copy_file

RE_FILENAMES = re.compile(
    r"""(?x) # Enable comments and ignore whitespace
(?P<domain>.*) # Domain part, can be arbitrary
-(?P<id>\d+) # Unique identifier for this sequence inside this domains
-(?P<set>\d+) # Group identifier for set of sequences
\.
(?P<full_extension>
    (?P<extension>[^\.]+(?:\.txt)?)
    \.xz # All files are always compressed
)
"""
)


def copy_files_hierarchical(files: t.List[str], to_path: str) -> t.List[str]:
    pool = mp.Pool()
    return pool.starmap(copy_single_file, map(lambda x: (x, to_path), files))


def copy_single_file(file: str, to_path: str) -> str:
    filename = path.basename(file)
    match = RE_FILENAMES.match(filename)
    if match is None:
        raise ValueError(f"Filename does not match regex: {filename}")
    new_path = path.join(to_path, match["set"], match["domain"], filename)
    os.makedirs(path.dirname(new_path), exist_ok=True)
    copy_file(file, new_path)
    return new_path


def postprocess_pcaps(files: t.List[str]) -> None:
    # Split long list into list of list
    # Inner lists are at most `elements_per_batch` long
    elements_per_batch = 50
    batches = [
        files[i * elements_per_batch : (i + 1) * elements_per_batch]
        for i in range(len(files) // elements_per_batch + 1)
    ]

    pool = mp.Pool()
    pool.map(postprocess_pcap_batch, batches)


def postprocess_pcap_batch(batch: t.List[str]) -> None:
    subprocess.check_call(
        ["./extract_sequence", "--convert-to-json"] + batch, stdout=subprocess.DEVNULL
    )
    for file in batch:
        os.remove(file)


def concat_xz_files(files: t.List[str]) -> str:
    contents = []
    for file in files:
        with lzma.open(file, "rt") as f:
            contents.append(f"# {file}")
            contents.append(f.read())
            contents.append("")

    return "\n".join(contents)


def main() -> None:
    args = sys.argv[1:]
    from_path = args[0]
    to_path = args[1]

    # Move pcap and dnstap files to new location
    new_file_names = copy_files_hierarchical(
        glob(path.join(from_path, "*", "*.pcap.xz")), to_path
    )
    copy_files_hierarchical(glob(path.join(from_path, "*", "*.dnstap.xz")), to_path)
    # Create one big tlskeys file for all keys
    tlskeys = concat_xz_files(glob(path.join(from_path, "*", "*.tlskeys.txt.xz")))
    with open(path.join(to_path, "tlskeys.txt"), "w+t") as f:
        f.write(tlskeys)

    # Postprocess pcaps
    postprocess_pcaps(new_file_names)


if __name__ == "__main__":
    main()
