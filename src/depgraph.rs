use chrome::{ChromeDebuggerMessage, Initiator, RedirectResponse, Request, Script};
use failure::{Error, ResultExt};
use petgraph::{graph::NodeIndex, Directed, Direction, Graph};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use {GraphExt, RequestInfo};

pub struct DepGraph {
    graph: Graph<RequestInfo, (), Directed>,
    nodes_cache: HashMap<String, NodeIndex>,
}

impl DepGraph {
    pub fn new() -> Self {
        let mut graph: Graph<_, _> = Graph::new();
        let mut nodes_cache: HashMap<String, NodeIndex> = HashMap::new();

        // Insert a node for "other" type dependencies
        // This should be the root node of everything
        nodes_cache.entry("other".to_string()).or_insert_with(|| {
            graph.add_node(RequestInfo {
                normalized_domain_name: "other".into(),
                requests: Vec::new(),
            })
        });

        DepGraph { graph, nodes_cache }
    }

    pub fn as_graph(&self) -> &Graph<RequestInfo, (), Directed> {
        &self.graph
    }

    pub fn into_graph(self) -> Graph<RequestInfo, (), Directed> {
        self.graph
    }

    pub fn process_messages(&mut self, messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
        self.do_process_messages(messages)?;
        self.graph.transitive_closure();
        Ok(())
    }

    fn do_process_messages(&mut self, messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
        let graph = RefCell::new(&mut self.graph);
        let nodes_cache = RefCell::new(&mut self.nodes_cache);

        // Create a new node and add it to the node cache
        let create_node = |msg: &ChromeDebuggerMessage| -> Result<NodeIndex, Error> {
            if let ChromeDebuggerMessage::NetworkRequestWillBeSent {
                request: Request { ref url, .. },
                ..
            } = *msg
            {
                let mut graph = graph.borrow_mut();
                let mut nodes_cache = nodes_cache.borrow_mut();

                let entry = nodes_cache.entry(url.clone()).or_insert_with(|| {
                    graph.add_node(RequestInfo::try_from(msg).expect(
                        "A requestWillBeSent must always be able to generate a valid node.",
                    ))
                });
                Ok(*entry)
            } else {
                bail!("Cannot create node from this message type.")
            }
        };
        // Find an existing node in the node cache by the URL
        let find_node = |url: String| -> Result<NodeIndex, Error> {
            let nodes_cache = nodes_cache.borrow();

            match nodes_cache.get(&*url) {
                Some(node) => Ok(*node),
                None => bail!(
                    "Cannot find node in cache even though there is a dependency to it: '{}'",
                    url
                ),
            }
        };
        let add_dependency = |from: NodeIndex, to: NodeIndex| {
            let mut graph = graph.borrow_mut();

            graph.update_edge(from, to, ());
        };

        for message in messages {
            use ChromeDebuggerMessage::NetworkRequestWillBeSent;
            if let NetworkRequestWillBeSent {
                request_id,
                initiator,
                redirect_response,
                ..
            } = message
            {
                let node = create_node(&message)?;

                // handle redirects
                if let Some(RedirectResponse { url }) = redirect_response {
                    let other = find_node(url.clone())
                        .with_context(|_| format_err!("Handling redirect, ID {}", request_id))?;
                    add_dependency(node, other);
                }

                // Add dependencies for node/msg combination
                match initiator {
                    Initiator::Other {} => {
                        let other = find_node("other".into())
                            .with_context(|_| format_err!("Handling other, ID {}", request_id))?;
                        add_dependency(node, other);
                    }
                    Initiator::Parser { ref url } => {
                        let other = find_node(url.clone())
                            .with_context(|_| format_err!("Handling parser, ID {}", request_id))?;
                        add_dependency(node, other);
                    }
                    Initiator::Script { ref stack } => {
                        fn traverse_stack<FN, AD>(
                            node: NodeIndex,
                            stack: &Script,
                            find_node: FN,
                            add_dependency: AD,
                        ) -> Result<(), Error>
                        where
                            FN: Fn(String) -> Result<NodeIndex, Error>,
                            AD: Fn(NodeIndex, NodeIndex),
                        {
                            for frame in &stack.call_frames {
                                let other = find_node(frame.url.clone())?;
                                add_dependency(node, other);
                            }
                            if let Some(parent) = &stack.parent {
                                traverse_stack(node, parent, find_node, add_dependency)?;
                            }

                            Ok(())
                        };

                        traverse_stack(node, stack, find_node, add_dependency)
                            .with_context(|_| format_err!("Handling script, ID {}", request_id))?;
                    }
                    }
                }
            }

        Ok(())
    }

    pub fn simplify_graph(&mut self) {
        // The number of requests between all nodes must be constant, otherwise we are not merging nodes correctly
        let request_count: usize = self.graph
            .raw_nodes()
            .iter()
            .map(|n| n.weight.requests.len())
            .sum();
        trace!("Number of requests in graph (start): {}", request_count);

        debug!(
            "Graph size (before simplify): {} / {}",
            self.graph.node_count(),
            self.graph.edge_count()
        );
        self.remove_self_loops();

        let mut needs_another_iteration = true;
        while needs_another_iteration {
            needs_another_iteration = false;

            needs_another_iteration |= self.remove_depends_on_same_domain();
            debug!(
                "Graph size (after same domain): {} / {}",
                self.graph.node_count(),
                self.graph.edge_count()
            );
            needs_another_iteration |= self.remove_dependency_subset();
            debug!(
                "Graph size (after dependency subset): {} / {}",
                self.graph.node_count(),
                self.graph.edge_count()
            );
        }

        let request_count_end: usize = self.graph
            .raw_nodes()
            .iter()
            .map(|n| n.weight.requests.len())
            .sum();
        trace!("Number of requests in graph (end): {}", request_count_end);
        assert_eq!(request_count, request_count_end);
    }

    fn remove_self_loops(&mut self) {
        // Keep all edges which return true
        // remove all others
        self.graph.retain_edges(|graph, edge_index| {
            if let Some((a, b)) = graph.edge_endpoints(edge_index) {
                // only keep those, which are not a self loop
                a != b
            } else {
                // should not happen, but just do nothing
                true
            }
        });
    }

    /// Returns true if some changes occured
    fn remove_depends_on_same_domain(&mut self) -> bool {
        let mut did_changes = false;

        let mut i = 0;
        'outer: while i < self.graph.node_count() {
            let node = NodeIndex::new(i);
            let node_domain = self.graph
                .node_weight(node)
                .expect("The node index is smaller than the node count")
                .normalized_domain_name
                .clone();

            let mut neighbors = self.graph.neighbors(node).detach();
            while let Some(other) = neighbors.next_node(&self.graph) {
                if node_domain == {
                    self.graph
                        .node_weight(other)
                        .expect("The other node index is smaller than the node count")
                        .normalized_domain_name
                        .clone()
                } {
                    did_changes = true;

                    // We do not need to transfer all the edges, because we calculated the transitive closure,
                    // meaning all the edges are already transfered
                    {
                        let (mut node_weight, other_weight) =
                            self.graph.index_twice_mut(node, other);
                        other_weight.merge_with(node_weight);
                    }
                    let _ = self.graph.remove_node(node).expect("Node id is valid");

                    // The current node is merged, thus we MUST abort the inner loop
                    // The nodes will be renumbered, thus at the current node index there will be a different node.
                    // We therefore skip the node index increment
                    continue 'outer;
                }
            }

            // increment node index
            i += 1;
        }

        did_changes
    }

    /// Returns true if some changes occured
    fn remove_dependency_subset(&mut self) -> bool {
        // If we have two nodes with equal domain but different dependencies,
        // we can remove the node with more dependencies, if the other node
        // has a strict subset of this nodes dependencies

        let mut did_changes = false;

        let mut i = 0;
        'outer: while i < self.graph.node_count() {
            let node_count = self.graph.node_count();
            let node = NodeIndex::new(i);
            let node_domain = self.graph
                .node_weight(node)
                .expect("The node index is smaller than the node count")
                .normalized_domain_name
                .clone();

            for j in 0..node_count {
                let other = NodeIndex::new(j);
                if node == other {
                    // do not test for same nodes
                    continue;
                }

                if node_domain == {
                    self.graph
                        .node_weight(other)
                        .expect("The other node index is smaller than the node count")
                        .normalized_domain_name
                        .clone()
                } {
                    let node_succs = self.graph.neighbors(node).collect::<HashSet<_>>();
                    let other_succs = self.graph.neighbors(other).collect::<HashSet<_>>();

                    if other_succs.is_subset(&node_succs) {
                        did_changes = true;

                        // The two nodes might be totally unrelated, meaning we need to first transfer all the incoming edges of `node` to `other`
                        let mut incomming = self.graph
                            .neighbors_directed(node, Direction::Incoming)
                            .detach();
                        while let Some(n) = incomming.next_node(&self.graph) {
                            self.graph.update_edge(n, other, ());
                        }

                        {
                            let (mut node_weight, other_weight) =
                                self.graph.index_twice_mut(node, other);
                            other_weight.merge_with(node_weight);
                        }
                        let _ = self.graph.remove_node(node).expect("Node id is valid");

                        // The current node is merged, thus we MUST abort the inner loop
                        // The nodes will be renumbered, thus at the current node index there will be a different node.
                        // We therefore skip the node index increment
                        continue 'outer;
                    }
                }
            }

            // increment node index
            i += 1;
        }

        did_changes
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use misc_utils::fs::file_open_read;
    use petgraph::{algo::is_isomorphic, graph::IndexType, EdgeType};
    use serde_json;
    use std::path::Path;

    fn get_messages<P>(path: P) -> Result<Vec<ChromeDebuggerMessage>, Error>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let rdr = file_open_read(path)
            .map_err(|err| format_err!("Opening input file '{}' failed: {}", path.display(), err))?;
        Ok(serde_json::from_reader(rdr).context("Failed to parse JSON")?)
    }

    fn test_graphs_are_isomorph<N1, N2, E1, E2, Ty, Ix>(
        expected: &Graph<N1, E1, Ty, Ix>,
        test: &Graph<N2, E2, Ty, Ix>,
    ) where
        Ty: EdgeType,
        Ix: IndexType,
    {
        assert_eq!(
            expected.node_count(),
            test.node_count(),
            "Node count must be equal"
        );
        assert_eq!(
            expected.edge_count(),
            test.edge_count(),
            "Edge count must be equal"
        );

        let pure_expected = expected.map(|_, _| (), |_, _| ());
        let pure_test = test.map(|_, _| (), |_, _| ());
        assert!(
            is_isomorphic(&pure_expected, &pure_test),
            "Graphs must be isomorphic"
        );
    }

    #[test]
    fn minimal_website_just_processing() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let favicon = expected_graph.add_node("favicon");
        let localhost_script = expected_graph.add_node("localhost script");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // deps on other
            (localhost, other),
            (favicon, other),
            // deps on localhost
            (localhost_script, localhost),
            (jquery, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc deps
            (fedora, localhost_script),
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .do_process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }

    #[test]
    fn minimal_website() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let favicon = expected_graph.add_node("favicon");
        let localhost_script = expected_graph.add_node("localhost script");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // self deps
            (other, other),
            (localhost, localhost),
            (favicon, favicon),
            (localhost_script, localhost_script),
            (jquery, jquery),
            (fedora, fedora),
            (google, google),
            (pythonhaven, pythonhaven),
            // deps on other
            (localhost, other),
            (favicon, other),
            (localhost_script, other),
            (jquery, other),
            (fedora, other),
            (google, other),
            (pythonhaven, other),
            // deps on localhost
            (localhost_script, localhost),
            (jquery, localhost),
            (fedora, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc deps
            (fedora, localhost_script),
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }

    #[test]
    fn minimal_website_remove_self_loops() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let favicon = expected_graph.add_node("favicon");
        let localhost_script = expected_graph.add_node("localhost script");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // deps on other
            (localhost, other),
            (favicon, other),
            (localhost_script, other),
            (jquery, other),
            (fedora, other),
            (google, other),
            (pythonhaven, other),
            // deps on localhost
            (localhost_script, localhost),
            (jquery, localhost),
            (fedora, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc deps
            (fedora, localhost_script),
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");
        depgraph.remove_self_loops();

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }

    #[test]
    fn minimal_website_remove_same_domain() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let favicon = expected_graph.add_node("favicon");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // deps on other
            (localhost, other),
            (favicon, other),
            (jquery, other),
            (fedora, other),
            (google, other),
            (pythonhaven, other),
            // deps on localhost
            (jquery, localhost),
            (fedora, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");
        depgraph.remove_self_loops();
        depgraph.remove_depends_on_same_domain();

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }

    #[test]
    fn minimal_website_remove_subset_deps() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // deps on other
            (localhost, other),
            (jquery, other),
            (fedora, other),
            (google, other),
            (pythonhaven, other),
            // deps on localhost
            (jquery, localhost),
            (fedora, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc deps
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");
        depgraph.remove_self_loops();
        depgraph.remove_dependency_subset();

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }

    #[test]
    fn minimal_website_simplify() {
        let mut expected_graph = Graph::<&'static str, ()>::new();
        let other = expected_graph.add_node("other");
        let localhost = expected_graph.add_node("localhost");
        let jquery = expected_graph.add_node("jquery");
        let fedora = expected_graph.add_node("fedora");
        let google = expected_graph.add_node("google");
        let pythonhaven = expected_graph.add_node("pythonhaven");

        expected_graph.extend_with_edges(&[
            // deps on other
            (localhost, other),
            (jquery, other),
            (fedora, other),
            (google, other),
            (pythonhaven, other),
            // deps on localhost
            (jquery, localhost),
            (fedora, localhost),
            (google, localhost),
            (pythonhaven, localhost),
            // misc deps
            (pythonhaven, jquery),
        ]);

        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
                .expect("Parsing the file must succeed."))
            .context("Failed to process all messages from chrome")
            .expect("A graph must be buildable from the data.");
        depgraph.simplify_graph();

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }
}
