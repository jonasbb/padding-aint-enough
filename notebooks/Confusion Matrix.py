# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.3'
#       jupytext_version: 1.3.4
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %%
# %matplotlib inline

# pylint: disable=redefined-outer-name

# %%
import json

import matplotlib.pyplot as plt
import numpy as np
from sklearn import datasets, svm
from sklearn.metrics import confusion_matrix
from sklearn.model_selection import train_test_split
from sklearn.utils.multiclass import unique_labels

# %%
# import some data to play with
iris = datasets.load_iris()
X = iris.data
y = iris.target
class_names = iris.target_names

# Split the data into a training set and a test set
X_train, X_test, y_train, y_test = train_test_split(X, y, random_state=0)

# Run classifier, using a model that is too regularized (C too low) to see
# the impact on the results
classifier = svm.SVC(kernel="linear", C=0.01)
y_pred = classifier.fit(X_train, y_train).predict(X_test)


def plot_confusion_matrix(
    y_true, y_pred, classes, normalize=False, title=None, cmap=plt.cm.Blues
):
    """
    This function prints and plots the confusion matrix.
    Normalization can be applied by setting `normalize=True`.
    """

    # Compute confusion matrix
    cm = confusion_matrix(y_true, y_pred, normalize=None)
    # Only use the labels that appear in the data
    classes = classes[unique_labels(y_true, y_pred)]
    if normalize:
        # cm = cm.astype("float") / cm.sum(axis=1)[:, np.newaxis]
        print("Normalized confusion matrix")
    else:
        print("Confusion matrix, without normalization")

    print(cm)

    fig, ax = plt.subplots()
    im = ax.imshow(cm, interpolation="nearest", cmap=cmap)
    ax.figure.colorbar(im, ax=ax)
    # We want to show all ticks...
    ax.set(
        xticks=np.arange(cm.shape[1]),
        yticks=np.arange(cm.shape[0]),
        # ... and label them with the respective list entries
        xticklabels=classes,
        yticklabels=classes,
        title=title,
        # ylabel="True label",
        # xlabel="Predicted label",
    )

    # Rotate the tick labels and set their alignment.
    plt.setp(ax.get_xticklabels(), rotation=45, ha="right", rotation_mode="anchor")

    # Loop over data dimensions and create text annotations.
    fmt = ".2f" if normalize else "d"
    thresh = cm.max() / 2.0
    for i in range(cm.shape[0]):
        for j in range(cm.shape[1]):
            if cm[i, j] != 0:
                ax.text(
                    j,
                    i,
                    format(cm[i, j], fmt),
                    ha="center",
                    va="center",
                    # color="white" if cm[i, j] > thresh else "black",
                    color="black" if 15 < cm[i, j] < 44 else "white",
                )
    fig.tight_layout()
    return ax


np.set_printoptions(precision=2)

# Plot non-normalized confusion matrix
plot_confusion_matrix(
    y_test, y_pred, classes=class_names, title="Confusion matrix, without normalization"
)

print("y_test", y_test)
print("y_pred", y_pred)
print(class_names)

# Plot normalized confusion matrix
plot_confusion_matrix(
    y_test,
    y_pred,
    classes=class_names,
    normalize=True,
    title="Normalized confusion matrix",
)


plot_confusion_matrix(
    [0, 0, 0, 1, 1, 1, 2, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    classes=class_names,
    normalize=True,
    title="Normalized confusion matrix",
)

plt.show()


# %%
# missclassifications_file = "../miss-commoncrawl.json"
missclassifications_file = "../miss-commoncrawl-51.json"
missclassifications = [json.loads(line) for line in open(missclassifications_file)]


# %%
# pairs of true domain and classifier result
def normalize(s: str) -> str:
    if s == "www.twitter.com":
        return "twitter.com"
    return s


classifications = [
    (mc["label"], normalize(mc["class_result"]["options"][0]["name"]))
    for mc in missclassifications
    # Ignore some domains
    if mc["label"] not in ["inta.gob.ar", "www.loveshack.org", "www.twitter.com"]
]

# %%
# domain2index = {
#     domain: i for i, domain in enumerate(set(domain for domain, _ in classifications))
# }
# domain2index

domain2index = {
    "www.aljazeera.net": 0,
    "www.amazon.com": 1,
    "www.bbc.co.uk": 2,
    "www.cnn.com": 3,
    "www.ebay.com": 4,
    "www.facebook.com": 5,
    "www.imdb.com": 6,
    # Kickass
    # "www.loveshack.org": 7,
    "www.rakuten.co.jp": 7,
    "www.reddit.com": 8,
    "www.rt.com": 9,
    "www.spiegel.de": 10,
    "stackoverflow.com": 11,
    "www.tmz.com": 12,
    "www.torproject.org": 13,
    "twitter.com": 14,
    "en.wikipedia.org": 15,
    "xhamster.com": 16,
    "www.xnxx.com": 17,
    # "www.twitter.com": 19,
}

domain2classlabel = {
    "www.aljazeera.net": "ALJAZEERA",
    "www.amazon.com": "AMAZON",
    "www.bbc.co.uk": "BBC",
    "www.cnn.com": "CNN",
    "www.ebay.com": "EBAY",
    "www.facebook.com": "FACEBOOK",
    "www.imdb.com": "IMDB",
    # Kickass
    # "www.loveshack.org": 7,
    "www.rakuten.co.jp": "RAKUTEN",
    "www.reddit.com": "REDDIT",
    "www.rt.com": "RT",
    "www.spiegel.de": "SPIEGEL",
    "stackoverflow.com": "STACKOVERFLOW",
    "www.tmz.com": "TMZ",
    "www.torproject.org": "TORPROJECT",
    "twitter.com": "TWITTER",
    "en.wikipedia.org": "WIKIPEDIA",
    "xhamster.com": "XHAMSTER",
    "www.xnxx.com": "XNXX",
    # "www.twitter.com": 19,
}

# %%
y_test = [domain2index[domain] for domain, _ in classifications]
y_pred = [domain2index[domain] for _, domain in classifications]

# %%
classes = [domain2classlabel[d] for d in domain2index.keys()]
plot_confusion_matrix(
    y_test,
    y_pred,
    classes=np.array(classes),
    # title="Subpage Classification",
    normalize=False,
    cmap=plt.cm.jet,
)

plt.gcf().set_size_inches(6, 5.5)
plt.tight_layout()
plt.savefig("subpage-confusion-matrix.svg")
plt.show()

# %%
np.array(list(domain2index.keys()))

# %%
