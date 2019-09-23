#!/usr/bin/env python3
import warnings  # isort:skip

warnings.filterwarnings("ignore", category=FutureWarning)  # NOQA
from tensorflow.python.util import deprecation  # isort:skip

deprecation._PRINT_DEPRECATION_WARNINGS = False  # NOQA

import csv
import datetime
import os
import typing as t
from glob import glob

import keras
import keras.utils
import numpy as np
import talos
from keras import layers
from keras.models import Sequential
from utils import Canonicalize, load_data, sanitize_file_name, shuffle_in_unison_scary

# Try replacing GRU, or SimpleRNN.
RNN = layers.LSTM
MASKING_VALUE = None
NUM_CLASSES = None

CONFUSION_DOMAINS_LISTS = [
    # "/home/jbushart/projects/confusion_domains/redirects.csv",
    "/home/jbushart/projects/encrypted-dns/results/2018-10-09-no-dnssec/confusion_domains.csv"
]


def main() -> None:
    global MASKING_VALUE, NUM_CLASSES
    data = load_data(CONFUSION_DOMAINS_LISTS, "/home/jbushart/tmp/", 8)
    print(data)
    data.assert_no_nan()

    MASKING_VALUE = data.masking_value
    NUM_CLASSES = data.classes

    p = {
        "activation": ["softmax"],
        "batch_size": [20, 40],
        "clipnorm": [0.1, 0.2, 0.4, 0.6, 0.8, 1.0],
        "dropout": [0, 0.05, 0.1, 0.15, 0.2, 0.25],
        "epochs": [100],
        "hidden_size": [128],
        "layers": [1],
        "recurrent_dropout": [0, 0.05, 0.1, 0.15, 0.2, 0.25],
        "optimizer": [keras.optimizers.Adam, keras.optimizers.Nadam],
    }

    scan_results = talos.Scan(
        x=data.training,
        y=data.training_labels,
        x_val=data.validation,
        y_val=data.validation_labels,
        params=p,
        model=test_model,
        experiment_name="Basic Sequences",
        fraction_limit=0.20,
        # random_method="quantum",
        reduction_method="correlation",
        reduction_metric="val_accuracy",
        print_params=True,
    )

    import IPython

    IPython.embed()


def test_model(
    x_train: np.array,
    y_train: np.array,
    x_val: np.array,
    y_val: np.array,
    params: t.Dict[str, t.Any],
) -> t.Tuple[int, int]:
    model = Sequential()
    model.add(layers.Masking(mask_value=MASKING_VALUE, input_shape=x_train.shape[1:]))
    # "Encode" the input sequence using an RNN, producing an output of HIDDEN_SIZE.
    # Note: In a situation where your input sequences have a variable length,
    # use input_shape=(None, num_feature).
    # model.add(RNN(HIDDEN_SIZE, return_sequences=True, activation="relu"))

    # The decoder RNN could be multiple layers stacked or a single layer.
    for _ in range(params["layers"] - 1):
        # By setting return_sequences to True, return not only the last output but
        # all the outputs so far in the form of (num_samples, timesteps,
        # output_dim). This is necessary as TimeDistributed in the below expects
        # the first dimension to be the timesteps.
        model.add(RNN(params["hidden_size"], return_sequences=True))
    model.add(RNN(params["hidden_size"], recurrent_dropout=params["recurrent_dropout"]))
    if params["dropout"] > 0:
        model.add(layers.Dropout(params["dropout"]))

    # Apply a dense layer to the every temporal slice of an input. For each of step
    # of the output sequence, decide which character should be chosen.
    model.add(layers.Dense(NUM_CLASSES, activation=params["activation"]))

    optimizer_args = {}
    if params["clipnorm"] is not None:
        optimizer_args["clipnorm"] = params["clipnorm"]
    optimizer = params["optimizer"](**optimizer_args)
    model.compile(
        loss="categorical_crossentropy",
        optimizer=optimizer,
        metrics=["accuracy"],
        # metrics=["categorical_accuracy", "accuracy"],
    )
    model.summary()

    run_name = (
        datetime.datetime.utcnow()
        .replace(microsecond=0, tzinfo=datetime.timezone.utc)
        .isoformat()
        + str(params)
    ).replace("/", "-")
    tensorboard = keras.callbacks.TensorBoard(
        log_dir=f"./tensorboardlogs/{run_name}/",
        histogram_freq=10,
        write_graph=True,
        write_grads=True,
        write_images=False,
        embeddings_freq=0,
        embeddings_layer_names=None,
        embeddings_metadata=None,
        embeddings_data=None,
        update_freq="epoch",
    )
    os.makedirs(f"./csvs/", exist_ok=True)
    csv_logger = keras.callbacks.CSVLogger(f"./csvs/{run_name}.csv", append=True)
    # checkpoints = keras.callbacks.ModelCheckpoint(
    #     "./checkpoints/model-{epoch:03d}.hdf5", period=100
    # )
    terminate_on_nan = keras.callbacks.TerminateOnNaN()

    # You have to shuffle categorical data manually
    # if also using the validation_split
    # https://github.com/keras-team/keras/issues/4298#issuecomment-258947029
    tr, la = shuffle_in_unison_scary(x_train, y_train)

    out = model.fit(
        tr,
        la,
        validation_data=(x_val, y_val),
        epochs=params["epochs"],
        # epochs=10,
        # steps_per_epoch=5,
        # validation_steps=5,
        shuffle=False,
        batch_size=params["batch_size"],
        callbacks=[csv_logger, tensorboard, terminate_on_nan],
    )

    return out, model


if __name__ == "__main__":
    main()
