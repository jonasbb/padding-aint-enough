#!/usr/bin/env python3
import argparse
import json
import typing as t
from urllib.parse import urlparse
import networkx as nx
import pydot

# first argument is graph name
GRAPH: pydot.Dot = pydot.Dot(
    "dependencies",
    graph_type="digraph",
    simplify=True,
    suppress_disconnected=False)
NODECACHE: t.Dict[str, pydot.Node] = dict()


class Resource(object):
    def __init__(self, name: str, **kwargs: t.Dict[t.Any, t.Any]) -> None:
        self.name = name
        self.dnsname = Resource._sanitize_name(name)
        self._depends_on: t.Set[Resource] = set()

    def __repr__(self) -> str:
        return self.name

    @staticmethod
    def _sanitize_name(name: str) -> str:
        if name.startswith("data:"):
            return "data"
        elif name == "other":
            return "other"

        tmp = urlparse(name).hostname
        if not tmp:
            print("-> " + name)
        return tmp

    def add_dependency(self, on: "Resource") -> None:
        if self == on:
            return

        self._depends_on.add(on)

    def get_dependencies(self) -> t.FrozenSet["Resource"]:
        return frozenset(self._depends_on)


class ResourceDependenciesFactory(object):
    def __init__(self, **kwargs: t.Dict[t.Any, t.Any]) -> None:
        self._resource_cache: t.Dict[str, Resource] = dict()

    def _get_resource(self, name: str) -> Resource:
        try:
            return self._resource_cache[name]
        except KeyError:
            resource = Resource(name)
            self._resource_cache[name] = resource
            return resource

    def create_dependency(self, from_res: str, to_res: str) -> None:
        from_ = self._get_resource(from_res)
        to_ = self._get_resource(to_res)
        from_.add_dependency(to_)

    def as_graph(self) -> None:
        graph = nx.DiGraph()
        graph.add_nodes_from(self._resource_cache.values())
        for resource in self._resource_cache.values():
            for other in resource.get_dependencies():
                graph.add_edge(resource, other)

        print(len(graph.nodes()), len(graph.edges()))

        def simplify_graph(graph):
            # try to simplify the graph
            for node in graph.nodes():
                succ = graph.successors(node)
                # merge two nodes if they both have the same DNS name and one depends on the other
                # since the dependency has the same DNS name, the DNS lookup will already have occured
                # it is therefore not helpful to keep the node
                if len(succ) == 1 and node.dnsname == succ[0].dnsname:
                    for pred in graph.predecessors(node):
                        graph.add_edge(pred, succ[0])
                    graph.remove_node(node)

            print("-------------------------------\nfirst simplify")
            print(len(graph.nodes()), len(graph.edges()))

            # TODO generalize to subset relationship between the dependencies
            # based on the transitive closure of the dependencies
            nodes = graph.nodes()
            for i in range(len(nodes)-1):
                node = nodes[i]
                # search for two unrelated nodes with same DNS name and same dependencies
                # same DNS name and same dependencies (by DNS name) are actually the same node
                for other in nodes[i+1:]:
                    if node.dnsname != other.dnsname:
                        continue
                    if node.get_dependencies() != other.get_dependencies():
                        continue

                    for pred in graph.predecessors(node):
                        graph.add_edge(pred, other)
                    graph.remove_node(node)
                    break

            print("-------------------------------\nsecond simplify")
            print(len(graph.nodes()), len(graph.edges()))

        size =(len(graph.nodes()), len(graph.edges()))
        newsize = (0, 0)
        while size != newsize:
            size = (len(graph.nodes()), len(graph.edges()))
            simplify_graph(graph)
            newsize = (len(graph.nodes()), len(graph.edges()))

        for node in graph.nodes():
            graph.node[node]['label'] = node.dnsname
            graph.node[node]['fullname'] = node.name
        nx.write_graphml(
            graph, "graph.graphml", encoding="utf-8", prettyprint=True)


FACTORY: ResourceDependenciesFactory = ResourceDependenciesFactory()


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
    global GRAPH, FACTORY  # pylint: disable=W0603
    FACTORY.create_dependency(resource, dependency)
    node = get_node(resource)
    dependency = get_node(dependency)
    # don't add self loops
    if node == dependency:
        return
    edge = pydot.Edge(dependency, node)
    GRAPH.add_edge(edge)


def parse_file(log: t.IO[str]) -> None:
    data = json.load(log)

    for elem in data:
        if elem["method"] != "Network.requestWillBeSent":
            continue
        fetch_url = elem["params"]["request"]["url"]
        initiator = elem["params"]["initiator"]
        add_dependencies_from_initiator(fetch_url, initiator)


def add_dependencies_from_initiator(resource: str,
                                    initiator: t.Dict[str, t.Any]) -> None:
    if initiator["type"] == "other":
        # redirects are type other and have a "redirectResponse"
        resource_depends_on(resource, "other")
    elif initiator["type"] == "parser":
        resource_depends_on(resource, initiator["url"])
    elif initiator["type"] == "script":
        add_dependencies_from_stack(resource, initiator["stack"])
    else:
        raise Exception(f"Unknown initiator type '{initiator['type']}'")


def add_dependencies_from_stack(resource: str,
                                stack: t.Dict[str, t.Any]) -> None:
    for frame in stack["callFrames"]:
        resource_depends_on(resource, frame["url"])

    if "parent" in stack:
        add_dependencies_from_stack(resource, stack["parent"])


def main() -> None:
    global GRAPH, FACTORY  # pylint: disable=W0603
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "-f", "--file", type=open, required=True, help="Log file to parse")
    args = parser.parse_args()

    init_special_nodes()
    parse_file(args.file)
    GRAPH.write("graph.png", format="png")
    FACTORY.as_graph()


if __name__ == '__main__':
    main()
