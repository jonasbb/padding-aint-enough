#!/usr/bin/env python3
import csv
import sys
import typing as t
from copy import copy
from glob import glob
from os import path

import keras
import numpy as np
import pylib
from keras import layers, utils
from keras.models import Sequential
from keras.preprocessing.sequence import pad_sequences

# Try replacing GRU, or SimpleRNN.
RNN = layers.LSTM
HIDDEN_SIZE = 128
BATCH_SIZE = 20
LAYERS = 3

# datapath = "/mnt/data/Downloads/new-task-setup/2018-10-01-no-dnssec/views/split0/"
datapath = "/home/jbushart/tmp/"

FAILED_DOMAINS_LIST = (
    "/mnt/data/Downloads/new-task-setup/2018-10-01-no-dnssec/failed_domains_final.csv"
)
CONFUSION_DOMAINS_LISTS = [
    # "/home/jbushart/projects/confusion_domains/redirects.csv",
    "/home/jbushart/projects/encrypted-dns/results/2018-10-09-no-dnssec/confusion_domains.csv"
]


class Canonicalize:
    cache: t.Dict[str, str]

    def __init__(self) -> None:
        self.cache = dict()
        # read all files and add them to the cache
        for file in CONFUSION_DOMAINS_LISTS:
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
    rdr = csv.reader(open(FAILED_DOMAINS_LIST, "r"))
    # skip header
    next(rdr)
    for file, _reason in rdr:
        res.add(sanitize_file_name(file))
    return res


def get_label(filename: str, canonicalizer: Canonicalize) -> str:
    # get the name of the directory containing the file
    label = path.basename(path.dirname(filename))
    return canonicalizer.canonicalize(label)


def shuffle_in_unison_scary(a: np.array, b: np.array) -> t.Tuple[np.array, np.array]:
    rng_state = np.random.get_state()
    np.random.shuffle(a)
    np.random.set_state(rng_state)
    np.random.shuffle(b)
    return (a, b)


def main() -> None:
    m = (0, 0)

    canonicalizer = Canonicalize()
    files_to_ignore = load_files_to_ignore()

    sequences = [
        [
            pylib.load_file(f)
            for f in glob(path.join(datapath, "*", f"*{i}-0.dnstap*"))
            if sanitize_file_name(f) not in files_to_ignore
        ]
        for i in range(10)
    ]
    training_raw = [[s.to_vector_encoding() for s in seqs] for seqs in sequences]
    labels = [[get_label(s.id(), canonicalizer) for s in seqs] for seqs in sequences]
    del sequences

    # find longest sequence
    longest_sequence = max(max(len(seq) for seq in x) for x in training_raw)

    training: t.List[np.array] = [
        pad_sequences(seqs, maxlen=longest_sequence, value=m, padding='post') for seqs in training_raw
    ]
    all_labels: t.Set[str] = set()
    for l in labels:
        all_labels = all_labels.union(set(l))
    label_to_num = {l: i for i, l in enumerate(all_labels)}

    labels_numeric = [[label_to_num[l] for l in ls] for ls in labels]
    labels_categorical = [
        utils.to_categorical(l, num_classes=len(all_labels)) for l in labels_numeric
    ]

    num_feature = len(training[0][0][0])
    num_classes = len(all_labels)
    del all_labels, labels_numeric, training_raw

    print("Build model...")
    model = Sequential()
    model.add(layers.Masking(mask_value=m, input_shape=(longest_sequence, num_feature)))
    # "Encode" the input sequence using an RNN, producing an output of HIDDEN_SIZE.
    # Note: In a situation where your input sequences have a variable length,
    # use input_shape=(None, num_feature).
    model.add(RNN(HIDDEN_SIZE, return_sequences=True, activation='relu'))
    # # As the decoder RNN's input, repeatedly provide with the last output of
    # # RNN for each time step. Repeat 'DIGITS + 1' times as that's the maximum
    # # length of output, e.g., when DIGITS=3, max output is 999+999=1998.
    # model.add(layers.RepeatVector(3))
    # The decoder RNN could be multiple layers stacked or a single layer.
    for _ in range(LAYERS - 1):
        # By setting return_sequences to True, return not only the last output but
        # all the outputs so far in the form of (num_samples, timesteps,
        # output_dim). This is necessary as TimeDistributed in the below expects
        # the first dimension to be the timesteps.
        model.add(RNN(HIDDEN_SIZE, return_sequences=True))
    model.add(RNN(HIDDEN_SIZE))

    # Apply a dense layer to the every temporal slice of an input. For each of step
    # of the output sequence, decide which character should be chosen.
    model.add(layers.Dense(num_classes, activation="softmax"))
    model.compile(
        loss="categorical_crossentropy", optimizer="adam", metrics=["accuracy"]
    )
    model.summary()

    tensorboard = keras.callbacks.TensorBoard(
        log_dir="./tensorboardlogs",
        histogram_freq=10,
        batch_size=BATCH_SIZE,
        write_graph=True,
        write_grads=True,
        write_images=False,
        embeddings_freq=0,
        embeddings_layer_names=None,
        embeddings_metadata=None,
        embeddings_data=None,
        update_freq="epoch",
    )

    val_data = np.concatenate((training[8], training[9]))
    val_labels = np.concatenate((labels_categorical[8], labels_categorical[9]))
    for r in range(200):
        for i in range(8):
            # You have to shuffle categorical data manually
            # if also using the validation_split
            # https://github.com/keras-team/keras/issues/4298#issuecomment-258947029
            tr, la = shuffle_in_unison_scary(
                copy(training[i]), copy(labels_categorical[i])
            )
            # model.fit(
            #     tr,
            #     la,
            #     validation_split=0.1,
            #     epochs=1,
            #     callbacks=[tensorboard],
            # )

            model.fit(
                tr,
                la,
                validation_data=(val_data, val_labels),
                initial_epoch=(r*8+i)*10,
                epochs=(r*8+i)*10+10,

                # epochs=10,
                # steps_per_epoch=5,
                # validation_steps=5,
                shuffle=False,
                batch_size=BATCH_SIZE,
                callbacks=[tensorboard],
            )

    import IPython

    IPython.embed()


if __name__ == "__main__":
    main()
