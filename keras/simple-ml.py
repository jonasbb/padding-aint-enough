#!/usr/bin/env python3
import warnings  # isort:skip

warnings.filterwarnings("ignore", category=FutureWarning)  # NOQA
from tensorflow.python.util import deprecation  # isort:skip

deprecation._PRINT_DEPRECATION_WARNINGS = False  # NOQA

import csv
import datetime
import os
import sys
import typing as t
from copy import copy
from glob import glob

import keras
import keras.utils
import numpy as np
import pylib
from keras import layers
from keras.models import Sequential
from keras.preprocessing.sequence import pad_sequences
from utils import Canonicalize, load_data, sanitize_file_name, shuffle_in_unison_scary

# Try replacing GRU, or SimpleRNN.
RNN = layers.LSTM
HIDDEN_SIZE = 128
# BATCH_SIZE = 5
BATCH_SIZE = None
LAYERS = 1
DROPOUT = 0.0
RECURRENT_DROPOUT = 0.0

CONFUSION_DOMAINS_LISTS = [
    # "/home/jbushart/projects/confusion_domains/redirects.csv",
    "/home/jbushart/projects/encrypted-dns/results/2018-10-09-no-dnssec/confusion_domains.csv"
]


def main() -> None:
    data = load_data(CONFUSION_DOMAINS_LISTS, "/home/jbushart/tmp/", 8)
    print(data)
    data.assert_no_nan()

    print("Build model...")
    model = Sequential()
    model.add(
        layers.Masking(
            mask_value=data.masking_value, input_shape=data.training.shape[1:]
        )
    )
    # "Encode" the input sequence using an RNN, producing an output of HIDDEN_SIZE.
    # Note: In a situation where your input sequences have a variable length,
    # use input_shape=(None, num_feature).
    # model.add(RNN(HIDDEN_SIZE, return_sequences=True, activation="relu"))

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
    # model.add(RNN(HIDDEN_SIZE,recurrent_dropout=RECURRENT_DROPOUT))
    model.add(
        RNN(HIDDEN_SIZE, recurrent_dropout=RECURRENT_DROPOUT)
    )  # , activation="relu"))
    if DROPOUT > 0:
        model.add(layers.Dropout(DROPOUT))

    # Apply a dense layer to the every temporal slice of an input. For each of step
    # of the output sequence, decide which character should be chosen.
    model.add(layers.Dense(data.classes, activation="softmax"))

    # model.compile(
    #     loss="categorical_crossentropy", optimizer="adam", metrics=["accuracy"]
    # )
    optimizer = keras.optimizers.Nadam(clipnorm=1.0)
    model.compile(
        loss="categorical_crossentropy",
        optimizer=optimizer,
        metrics=["categorical_accuracy", "accuracy"],
    )
    model.summary()

    isodate = (
        datetime.datetime.utcnow()
        .replace(microsecond=0, tzinfo=datetime.timezone.utc)
        .isoformat()
    )
    tensorboard = keras.callbacks.TensorBoard(
        log_dir=f"./tensorboardlogs/{isodate}/",
        histogram_freq=10,
        # batch_size=BATCH_SIZE,
        write_graph=True,
        write_grads=True,
        write_images=False,
        embeddings_freq=0,
        embeddings_layer_names=None,
        embeddings_metadata=None,
        embeddings_data=None,
        update_freq="epoch",
    )
    os.makedirs(f"./csvs/")
    csv_logger = keras.callbacks.CSVLogger(f"./csvs/{isodate}.csv", append=True)
    checkpoints = keras.callbacks.ModelCheckpoint(
        "./checkpoints/model-{epoch:03d}.hdf5", period=100
    )
    terminate_on_nan = keras.callbacks.TerminateOnNaN()

    for r in range(15):
        # You have to shuffle categorical data manually
        # if also using the validation_split
        # https://github.com/keras-team/keras/issues/4298#issuecomment-258947029
        tr, la = shuffle_in_unison_scary(
            copy(data.training), copy(data.training_labels)
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
            validation_data=(data.validation, data.validation_labels),
            initial_epoch=r * 10,
            epochs=r * 10 + 10,
            # epochs=10,
            # steps_per_epoch=5,
            # validation_steps=5,
            shuffle=False,
            batch_size=BATCH_SIZE,
            callbacks=[csv_logger, tensorboard, terminate_on_nan],
        )


if __name__ == "__main__":
    main()
