#!/usr/bin/env python3
import argparse
import json
import typing as t
from urllib.parse import urlparse

import pydot

# first argument is graph name
GRAPH: pydot.Dot = pydot.Dot(
    "dependencies",
    graph_type="digraph",
    simplify=True,
    suppress_disconnected=False)
NODECACHE: t.Dict[str, pydot.Node] = dict()


def node_sanitize_name(name: str) -> str:
    if name.startswith("data:"):
        return "\"data\""
    elif name == "other":
        return "\"other\""

    name = urlparse(name).hostname
    # name = name.replace("https://", "").replace("http://", "")[:100]
    return f"\"{name}\""


# set_style and set_color cause problems with pylint
# pylint: disable=E1101
def init_special_nodes() -> None:
    global GRAPH, NODECACHE  # pylint: disable=W0603

    # "other" node for all requests with initiator other
    name = node_sanitize_name("other")
    data_node = pydot.Node(name)
    data_node.set_style("filled")
    data_node.set_color("red")
    NODECACHE[name] = data_node
    GRAPH.add_node(data_node)

    # "data" node for data URIs
    name = node_sanitize_name("data:")
    data_node = pydot.Node(name)
    data_node.set_style("filled")
    data_node.set_color("green")
    NODECACHE[name] = data_node
    GRAPH.add_node(data_node)


def get_node(hostname: str) -> pydot.Node:
    global GRAPH, NODECACHE  # pylint: disable=W0603
    hostname = node_sanitize_name(hostname)
    try:
        return NODECACHE[hostname]
    except KeyError:
        node = pydot.Node(hostname)
        GRAPH.add_node(node)
        NODECACHE[hostname] = node
        return node


def resource_depends_on(resource: str, dependency: str) -> None:
    global GRAPH  # pylint: disable=W0603
    node = get_node(resource)
    dependency = get_node(dependency)
    # don't add self loops
    if node == dependency:
        return
    edge = pydot.Edge(dependency, node)
    GRAPH.add_edge(edge)


def parse_file(log: t.IO[str]) -> None:
    data = map(json.loads, log.readlines())

    for elem in data:
        if elem["method"] != "Network.requestWillBeSent":
            continue
        fetch_url = elem["params"]["request"]["url"]
        initiator = elem["params"]["initiator"]
        add_dependencies_from_initiator(fetch_url, initiator)


def add_dependencies_from_initiator(resource: str,
                                    initiator: t.Dict[str, t.Any]) -> None:
    print(initiator)
    if initiator["type"] == "other":
        resource_depends_on(resource, "other")
    elif initiator["type"] == "parser":
        resource_depends_on(resource, initiator["url"])
    elif initiator["type"] == "script":
        for frame in initiator["stack"]["callFrames"]:
            resource_depends_on(resource, frame["url"])
    else:
        raise Exception(f"Unknown initiator type '{initiator['type']}'")


def main() -> None:
    global GRAPH  # pylint: disable=W0603
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "-f", "--file", type=open, required=True, help="Log file to parse")
    args = parser.parse_args()

    init_special_nodes()
    parse_file(args.file)
    GRAPH.write("graph.png", format="png")


if __name__ == '__main__':
    main()
