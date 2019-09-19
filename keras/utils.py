import warnings  # isort:skip

warnings.filterwarnings("ignore", category=FutureWarning)  # NOQA

import csv
import os.path
import sys
import typing as t
from dataclasses import dataclass
from glob import glob

import keras
import keras.utils
import numpy as np
import pylib
from keras.preprocessing.sequence import pad_sequences


@dataclass(init=True)
class SequenceData:
    training: np.array
    training_labels: np.array
    validation: np.array
    validation_labels: np.array
    masking_value: t.Any
    classes: int

    def __repr__(self) -> str:
        return f"""{self.__class__.__name__}(
    training: {self.training.shape},
    training_labels: {self.training_labels.shape},
    validation: {self.validation.shape},
    validation_labels: {self.validation_labels.shape},
    masking_value: {self.masking_value},
    classes: {self.classes},
)"""

    def assert_no_nan(self) -> None:
        if np.any(np.isnan(self.training)):
            raise ValueError(f"NaN found in training data")
        if np.any(np.isnan(self.training_labels)):
            raise ValueError(f"NaN found in training labels")
        if np.any(np.isnan(self.validation)):
            raise ValueError(f"NaN found in validation data")
        if np.any(np.isnan(self.validation_labels)):
            raise ValueError(f"NaN found in validation labels")


def sanitize_file_name(filename: str) -> str:
    """
    Strips all superflous parts of the filename and returns the identifier of the Sequence
    """

    # strip extension
    tmp = os.path.basename(filename)
    if tmp.endswith(".xz"):
        tmp = tmp[: -len(".xz")]
    if tmp.endswith(".json"):
        tmp = tmp[: -len(".json")]
    if tmp.endswith(".dnstap"):
        tmp = tmp[: -len(".dnstap")]
    return sys.intern(tmp)


def shuffle_in_unison_scary(a: np.array, b: np.array) -> t.Tuple[np.array, np.array]:
    """
    Shuffles two numpy arrays identically

    The shuffle modifies the input arguments.
    """
    rng_state = np.random.get_state()
    np.random.shuffle(a)
    np.random.set_state(rng_state)
    np.random.shuffle(b)
    return (a, b)


class Canonicalize:
    # All strings in this dict need to be interned already
    cache: t.Dict[str, str]

    def __init__(self, confusion_domains: t.List[str]) -> None:
        self.cache = dict()
        # read all files and add them to the cache
        for file in confusion_domains:
            rdr = csv.reader(open(file, "r"))
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
        """
        Return the canonical representation of a domain

        This takes care of determining the correct label for a domain including redirects and manual canonicalizations.
        """

        res = domain
        try:
            while True:
                res = self.cache[res]
        except KeyError:
            res = sys.intern(res)
        return res

    def canonicalize_path(self, path: str) -> str:
        """
        Returns the canonical label for a full path to a file.
        """

        # get the name of the directory containing the file
        label = os.path.basename(os.path.dirname(path))
        return self.canonicalize(label)


def load_data(
    confusion_domains: t.List[str], datapath: str, training_validation_split: int
) -> SequenceData:
    """
    `training_validation_split`: The first ID which should be used for validation instead of training.
    """
    canonicalizer = Canonicalize(confusion_domains)

    sequences: t.List[t.List[pylib.Sequence]] = []
    for i in range(10):
        print(f"Load shard {i} of DNS Sequences...")
        sequences.append(
            [
                pylib.load_file(f)
                for f in glob(os.path.join(datapath, "*", f"*{i}-0.pcap*"))
            ]
        )
    # Split into ML-ready data and labels
    training_raw = [[s.to_vector_encoding() for s in seqs] for seqs in sequences]
    labels = [
        [canonicalizer.canonicalize_path(s.id()) for s in seqs] for seqs in sequences
    ]
    del sequences

    # find longest sequence
    longest_sequence = max(max(len(seq) for seq in x) for x in training_raw)
    padding_value = tuple([0] * len(training_raw[0][0][0]))
    distinct_categories = len(training_raw[0])
    print(
        f"""Longest Sequence {longest_sequence}
Padding Value: {padding_value}
Distinct Categories: {distinct_categories}
"""
    )

    print("Create trainings and validation sets")
    # Create numpy arrays for training and validation
    training: np.array = pad_sequences(
        [s for seqs in training_raw[:training_validation_split] for s in seqs],
        maxlen=longest_sequence,
        value=padding_value,
        padding="post",
    )
    validation: np.array = pad_sequences(
        [s for seqs in training_raw[training_validation_split:] for s in seqs],
        maxlen=longest_sequence,
        value=padding_value,
        padding="post",
    )

    expected_shape = (
        distinct_categories * training_validation_split,
        longest_sequence,
        2,
    )
    if training.shape != expected_shape:
        raise Exception(
            f"There was an error converting the sequences into a numpy array: Expected shape {expected_shape} but found shape {training.shape}"
        )

    # Convert the labels into numbers and then into categorical information
    all_labels: t.Set[str] = set()
    for l in labels:
        all_labels = all_labels.union(set(l))
    label_to_num = {l: i for i, l in enumerate(all_labels)}

    training_labels_numeric = [
        label_to_num[l] for ls in labels[:training_validation_split] for l in ls
    ]
    validation_labels_numeric = [
        label_to_num[l] for ls in labels[training_validation_split:] for l in ls
    ]
    training_labels = keras.utils.to_categorical(
        training_labels_numeric, num_classes=len(all_labels)
    )
    validation_labels = keras.utils.to_categorical(
        validation_labels_numeric, num_classes=len(all_labels)
    )

    expected_shape_labels = (training.shape[0], distinct_categories)
    if training_labels.shape != expected_shape_labels:
        raise Exception(
            f"There was an error converting the labels into a numpy array: Expected shape {expected_shape_labels} but found shape {training_labels.shape}"
        )

    return SequenceData(
        training=training,
        training_labels=training_labels,
        validation=validation,
        validation_labels=validation_labels,
        masking_value=padding_value,
        classes=len(all_labels),
    )


if __name__ == "__main__":
    load_data([], "/home/jbushart/tmp/", 5)
