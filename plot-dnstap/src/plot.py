#!/usr/bin/env python3
# pylint: disable=redefined-outer-name
import json
import sys
import typing as t
from datetime import datetime

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.colors import ListedColormap
from matplotlib.lines import Line2D

if "queries" not in dir():
    queries: str = "[]"
if "image_width" not in dir():
    image_width: int = 10
if "image_height" not in dir():
    image_height: int = 6
if "output_filename" not in dir():
    output_filename: t.Optional[str] = None

# Need to always clear the figure as otherwise the state is kept over multiple invocations of this
# script
plt.clf()


def parse_date(date: str) -> datetime:
    """
    Convert an ISO8601 string from chrono into a datetime object
    """

    # See bug: https://bugs.python.org/issue35829
    return datetime.fromisoformat(date.replace("Z", "+00:00"))


def categorical_cmap(
    num_colors: int, colors_steps: int, cmap: str = "tab10", continuous: bool = False
) -> ListedColormap:

    if num_colors > plt.get_cmap(cmap).N:
        raise ValueError("Too many categories for colormap.")
    if continuous:
        ccolors = plt.get_cmap(cmap)(np.linspace(0, 1, num_colors))
    else:
        ccolors = plt.get_cmap(cmap)(np.arange(num_colors, dtype=int))
    cols = np.zeros((num_colors * colors_steps, 3))
    for i, c in enumerate(ccolors):
        chsv = matplotlib.colors.rgb_to_hsv(c[:3])
        arhsv = np.tile(chsv, colors_steps).reshape(colors_steps, 3)
        arhsv[:, 1] = np.linspace(chsv[1], 0.25, colors_steps)
        arhsv[:, 2] = np.linspace(chsv[2], 1, colors_steps)
        rgb = matplotlib.colors.hsv_to_rgb(arhsv)
        cols[i * colors_steps : (i + 1) * colors_steps, :] = rgb
    cmap = ListedColormap(cols)
    return cmap


def info_from_source(
    colormap: ListedColormap, source: str, response_size: int, queryset_id: int
) -> t.Tuple[str, float, float, t.Optional[str]]:
    """
    Return some plotting parameters based on the query source and response size.
    """
    height = None
    color = None
    alpha = None
    hatch = None
    colorcount = len(colormap.colors) // 4
    if source == "Forwarder":
        height = 1.0
        alpha = 1.0
        if response_size <= 1 * 468:
            # red
            color = colormap.colors[colorcount * 0 + queryset_id]
        elif response_size <= 2 * 468:
            # yellow
            color = colormap.colors[colorcount * 1 + queryset_id]
        elif response_size <= 3 * 468:
            # magenta
            color = colormap.colors[colorcount * 2 + queryset_id]
        elif response_size <= 4 * 468:
            # blue
            color = colormap.colors[colorcount * 3 + queryset_id]
        else:
            # black
            color = "k"
    elif source == "Client":
        height = 0.5
        color = "g"
        alpha = 0.33
    else:
        height = 0.33
        color = "crimson"
        alpha = 0.5

    return (color, height, alpha, hatch)


LABEL2INDEX: t.Dict[str, int] = dict()

# Only for use by `get_next_index`
NEXT_INDEX = -1


def get_next_index() -> int:
    """
    Return an index to use as axis argument for matplotlib
    """

    global NEXT_INDEX  # pylint: disable=global-statement
    tmp = NEXT_INDEX
    NEXT_INDEX -= 1
    return tmp


# class Query(t.TypedDict):
#     source: str
#     qname: str
#     qtype: str
#     start: datetime
#     end: datetime
#     query_size: int
#     response_size: int


parsed_queries: t.List[t.List[t.Any]] = json.loads(queries)
if len(parsed_queries) == 0:
    sys.exit(1)

num_querysets = len(parsed_queries)
colormap = categorical_cmap(4, num_querysets)

# To separate the different domains better pretend we have one additional queryset
# such that this always creates an empty line
if num_querysets > 1:
    num_querysets += 1

for queryset_id, (queryset, filename) in enumerate(parsed_queries):
    # A queryset is the set of queries from a single source file
    # Multiple sourcefiles can be combined into one output plot
    for q in queryset:
        q["start"] = parse_date(q["start"])
        q["end"] = parse_date(q["end"])

    # Sort by start time
    queryset.sort(key=lambda x: x["start"])

    min_dns_start = queryset[0]["start"]
    prev_end = None

    for i, q in enumerate(queryset):
        # plt.plot([i, i], label=q["qname"])

        label = f"{q['qname']} ({q['qtype']})"

        if label not in LABEL2INDEX.keys():
            LABEL2INDEX[label] = get_next_index()
        ind = LABEL2INDEX[label]

        # The ind only contains which qname/qtype pair we have
        # Since we can have multiple querysets we want them to be plotted under each other
        # So set 0 on the top, set 1 afterwards, and only then we want to plot ind=1
        # This scales ind to do the right thing
        ind = ind * num_querysets - queryset_id

        start = q["start"]
        end = q["end"]
        color, height, alpha, hatch = info_from_source(
            colormap, q["source"], q["response_size"], queryset_id
        )

        plt.barh(
            ind,
            (end - start).total_seconds(),
            left=(start - min_dns_start).total_seconds(),
            color=color,
            alpha=alpha,
            height=height,
            hatch=hatch,
        )

        if num_querysets == 1:
            # Only attach labels, if we print a single queryset
            label = ""
            if prev_end:
                label += f" 𝚫 {round((end-prev_end).total_seconds() * 1000, 3)} ms "
            prev_end = end
            label += f"\n⏱ {round((end-start).total_seconds() * 1000, 3)} ms"
            plt.text(
                (end - min_dns_start).total_seconds(),
                ind,
                label,
                horizontalalignment="left",
                verticalalignment="center",
                fontname="Symbola",
            )

legend_handles = [
    Line2D(
        [0],
        [0],
        color=info_from_source(colormap, "Forwarder", 128, queryset_id)[0],
        lw=8,
        label=filename,
    )
    for queryset_id, (_queries, filename) in enumerate(parsed_queries)
]

yticks: t.List[float] = []
yticks_labels: t.List[str] = []
for (label, ind) in LABEL2INDEX.items():
    yticks.append(ind * num_querysets - num_querysets / 2)
    yticks_labels.append(label)
plt.yticks(yticks, yticks_labels)
plt.xlabel("Time in seconds")

plt.legend(
    handles=legend_handles,
    # loc="upper left",
    # bbox_to_anchor=(0, 0),
    fancybox=True,
)

plt.gcf().set_size_inches(image_width, image_height)
plt.tight_layout()
if output_filename:
    plt.savefig(output_filename)
