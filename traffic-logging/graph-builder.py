#!/usr/bin/env python3
import argparse
import json
import typing as t
from urllib.parse import urlparse
import networkx as nx


class Resource(object):
    def __init__(self, name: str, index: int,
                 **kwargs: t.Dict[t.Any, t.Any]) -> None:
        self.name = name
        self.dnsname = Resource._sanitize_name(name)
        self.index = index
        self._depends_on: t.Set[Resource] = set()

    def __repr__(self) -> str:
        return f"{self.name} ({self.index})"

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
        self._idx = 0

    def _get_resource(self, name: str) -> Resource:
        try:
            return self._resource_cache[name]
        except KeyError:
            self._idx += 1
            resource = Resource(name, self._idx)
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
        graph = nx.transitive_closure(graph)

        print(len(graph.nodes()), len(graph.edges()))

        def simplify_graph(graph: nx.DiGraph) -> None:
            print("-------------------------------\nfirst simplify")
            # Remove all nodes which depend on a node with identical DNS name
            # the second one would never cause a new DNS lookup
            for node in graph.nodes():
                if node.name == "https://www.redditstatic.com/droparrowgray.gif":
                    print(node, len(graph.successors(node)))
                for succ in graph.successors_iter(node):
                    if node.dnsname == succ.dnsname:
                        # No need to copy the edges as we already have computed the transitive closure
                        graph.remove_node(node)
                        break
            print(len(graph.nodes()), len(graph.edges()))

            print("-------------------------------\nsecond simplify")
            # TODO generalize to subset relationship between the dependencies
            # based on the transitive closure of the dependencies
            nodes = graph.nodes()
            for i in range(len(nodes) - 1):
                node = nodes[i]
                # search for two unrelated nodes with same DNS name and same dependencies
                # same DNS name and same dependencies (by DNS name) are actually the same node
                for other in nodes[i + 1:]:
                    if node.dnsname != other.dnsname:
                        continue

                    # There are different possibilities here
                    #
                    # Equal dependencies is the easy case.
                    if graph.successors(node) != graph.successors(other):
                        continue

                    for pred in graph.predecessors(node):
                        graph.add_edge(pred, other)
                    graph.remove_node(node)
                    break

            print(len(graph.nodes()), len(graph.edges()))

        size = (len(graph.nodes()), len(graph.edges()))
        newsize = (0, 0)
        while size != newsize:
            size = (len(graph.nodes()), len(graph.edges()))
            simplify_graph(graph)
            newsize = (len(graph.nodes()), len(graph.edges()))

        print("Names with multiple nodes:")
        import collections
        c = collections.Counter(node.dnsname for node in graph.nodes_iter())
        for name, count in c.most_common():
            if count > 1:
                print("  ", name, count)

        for node in graph.nodes():
            graph.node[node]['label'] = node.dnsname
            graph.node[node]['fullname'] = node.name
            graph.node[node]['index'] = str(node.index)
            graph.node[node]['deps'] = repr(node.get_dependencies())
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


def resource_depends_on(resource: str, dependency: str) -> None:
    global FACTORY  # pylint: disable=W0603
    assert resource and len(resource) > 0
    if dependency == "":
        return
    FACTORY.create_dependency(resource, dependency)


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
    # TODO maybe carry some identifier as the request ID around
    if initiator["type"] == "other":
        # TODO redirects are type other and have a "redirectResponse"
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

    parse_file(args.file)
    FACTORY.as_graph()


if __name__ == '__main__':
    main()
