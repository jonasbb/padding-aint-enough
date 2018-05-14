use chrome::{ChromeDebuggerMessage, Initiator, RedirectResponse, Request, StackTrace};
use failure::{Error, ResultExt};
use petgraph::{graph::NodeIndex, Directed, Direction, Graph};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use {GraphExt, RequestInfo};

pub struct DepGraph {
    graph: Graph<RequestInfo, (), Directed>,
}

impl DepGraph {
    pub fn new(messages: &[ChromeDebuggerMessage]) -> Result<Self, Error> {
        let mut graph = DepGraph::process_messages(messages)?;
        graph.transitive_closure();

        Ok(DepGraph { graph })
    }

    pub fn as_graph(&self) -> &Graph<RequestInfo, (), Directed> {
        &self.graph
    }

    pub fn into_graph(self) -> Graph<RequestInfo, (), Directed> {
        self.graph
    }

    fn build_script_id_cache(
        messages: &[ChromeDebuggerMessage],
    ) -> Result<HashMap<String, HashSet<String>>, Error> {
        let mut script_id_cache: HashMap<String, HashSet<String>> = HashMap::new();

        {
            let script_id_cache = RefCell::new(&mut script_id_cache);

            // Find for a script ID all the URLs it depends on
            let find_script_deps = |script_id: &str| -> Result<HashSet<String>, Error> {
                let script_id_cache = script_id_cache.borrow();
                match script_id_cache.get(script_id) {
                    Some(deps) => Ok(deps.clone()),
                    None => bail!(
                        "Could not find any dependencies for script with ID {}",
                        script_id
                    ),
                }
            };

            // The events Debugger.scriptParsed and Debugger.scriptFailedToParse might only appear after a network request, which has the script in the stack trace.
            // Parse them before everything else such that the script_ids can be used for the network requests parts
            for message in messages {
                use ChromeDebuggerMessage::{DebuggerScriptFailedToParse, DebuggerScriptParsed};
                match message {
                    DebuggerScriptParsed {
                        script_id,
                        url,
                        stack_trace,
                    }
                    | DebuggerScriptFailedToParse {
                        script_id,
                        url,
                        stack_trace,
                    } => {
                        fn traverse_stack<FSD>(
                            stack: &StackTrace,
                            find_script_deps: FSD,
                            mut url_deps_accu: HashSet<String>,
                        ) -> Result<HashSet<String>, Error>
                        where
                            FSD: Fn(&str) -> Result<HashSet<String>, Error>,
                        {
                            for frame in &stack.call_frames {
                                if frame.url == "" {
                                    let deps =
                                        find_script_deps(&*frame.script_id).with_context(|_| {
                                            format_err!(
                                                "Cannot get script dependencies for script ID {}",
                                                frame.script_id,
                                            )
                                        })?;
                                    url_deps_accu.extend(deps);
                                } else {
                                    url_deps_accu.insert(frame.url.clone());
                                }
                            }
                            if let Some(parent) = &stack.parent {
                                url_deps_accu =
                                    traverse_stack(parent, find_script_deps, url_deps_accu)?;
                            }

                            Ok(url_deps_accu)
                        };

                        // some scripts do not have a stacktrace, skip them
                        if let Some(stack_trace) = stack_trace {
                            // Chrome contains special case URLs like "extensions::event_bindings"
                            // They all start with "extensions::", so skip them
                            if !url.starts_with("extensions::") {
                                let mut deps =
                                    traverse_stack(stack_trace, find_script_deps, HashSet::new())
                                        .with_context(|_| {
                                            format_err!(
                                            "Handling script (failed to) parse event, Script ID {}",
                                            script_id
                                        )
                                        })?;
                                script_id_cache.borrow_mut().insert(script_id.clone(), deps);
                            }
                        }
                    }

                    // ignore other events
                    _ => {}
                }
            }
        }

        Ok(script_id_cache)
    }

    fn process_messages(
        messages: &[ChromeDebuggerMessage],
    ) -> Result<Graph<RequestInfo, ()>, Error> {
        let script_id_cache = DepGraph::build_script_id_cache(messages)
            .context("Failed to build the scrip_id_cache.")?;
        let mut graph = Graph::new();
        let mut nodes_cache: HashMap<String, NodeIndex> = HashMap::new();

        // Insert a node for "other" type dependencies
        // This should be the root node of everything
        nodes_cache.entry("other".to_string()).or_insert_with(|| {
            graph.add_node(RequestInfo {
                normalized_domain_name: "other".into(),
                requests: Vec::new(),
            })
        });

        {
            let graph = RefCell::new(&mut graph);
            let nodes_cache = RefCell::new(&mut nodes_cache);

            // Create a new node and add it to the node cache
            // Do not create a node if it is a data URI
            let create_node = |msg: &ChromeDebuggerMessage| -> Result<Option<NodeIndex>, Error> {
                if let ChromeDebuggerMessage::NetworkRequestWillBeSent {
                    request: Request { ref url, .. },
                    ..
                } = *msg
                {
                    Ok(if url.starts_with("data:") {
                        None
                    } else {
                        let mut graph = graph.borrow_mut();
                        let mut nodes_cache = nodes_cache.borrow_mut();

                        let entry = nodes_cache.entry(url.clone()).or_insert_with(|| {
                            graph.add_node(RequestInfo::try_from(msg).expect(
                                "A requestWillBeSent must always be able to generate a valid node.",
                            ))
                        });
                        Some(*entry)
                    })
                } else {
                    bail!("Cannot create node from this message type.")
                }
            };
            // Find for a script ID all the URLs it depends on
            let find_script_deps = |script_id: &str| -> Result<HashSet<String>, Error> {
                match script_id_cache.get(script_id) {
                    Some(deps) => Ok(deps.clone()),
                    None => bail!(
                        "Could not find any dependencies for script with ID {}",
                        script_id
                    ),
                }
            };
            // Add dependencies to the node `node`
            // Uses the URL to lookup the node with for this URL
            // If this fails, it uses the script ID to lookup all the URL this script ID depends on
            // Adds all the found URLs as dependencies to the node.
            let add_dependencies_to_node = |node: NodeIndex,
                                            url: &str,
                                            script_id: Option<&str>|
             -> Result<(), Error> {
                let nodes_cache = nodes_cache.borrow();
                let mut graph = graph.borrow_mut();

                // convert a single URL to a NodeIndex
                let url2node = |url: &str| -> Result<NodeIndex, Error> {
                    nodes_cache
                        .get(url)
                        .cloned()
                        .ok_or_else(|| format_err!("Could not find URL '{}' in cache", url))
                };

                if let Ok(dep) = url2node(url) {
                    // if URL succeeds, then everything is fine
                    graph.update_edge(node, dep, ());
                } else if let Some(script_id) = script_id {
                    // Lookup all the dependend URLs for the script
                    // Convert them into NodeIndex (via node_cache)
                    // Add them all as dependencies
                    find_script_deps(script_id)
                        .with_context(|_| format_err!("Failed to lookup script ID {}", script_id))?
                        .into_iter()
                        .map(|url| url2node(&*url))
                        .collect::<Result<Vec<_>, Error>>()
                        .context("Failed to convert a URL dependency of a script to a node.")?
                        .into_iter()
                        .for_each(|dep| {
                            graph.update_edge(node, dep, ());
                        });
                } else {
                    bail!(
                        "Could not find URL '{}' in cache and script ID is missing",
                        url
                    )
                }
                Ok(())
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
                    let node = match create_node(&message)? {
                        Some(node) => node,
                        // skip creation of data URIs
                        None => continue,
                    };

                    // handle redirects
                    if let Some(RedirectResponse { url }) = redirect_response {
                        add_dependencies_to_node(node, url, None)
                            .with_context(|_| format_err!("Handling redirect, ID {}", request_id))?;
                    }

                    // Add dependencies for node/msg combination
                    match initiator {
                        Initiator::Other {} => {
                            add_dependencies_to_node(node, "other", None).with_context(|_| {
                                format_err!("Handling other, ID {}", request_id)
                            })?;
                        }
                        Initiator::Parser { ref url } => {
                            add_dependencies_to_node(node, url, None).with_context(|_| {
                                format_err!("Handling parser, ID {}", request_id)
                            })?;
                        }
                        Initiator::Script { ref stack } => {
                            fn traverse_stack<ADTN>(
                                node: NodeIndex,
                                stack: &StackTrace,
                                add_dependencies_to_node: ADTN,
                            ) -> Result<(), Error>
                            where
                                ADTN: Fn(NodeIndex, &str, Option<&str>) -> Result<(), Error>,
                            {
                                for frame in &stack.call_frames {
                                    add_dependencies_to_node(
                                        node,
                                        &*frame.url,
                                        Some(&*frame.script_id),
                                    )?;
                                }
                                if let Some(parent) = &stack.parent {
                                    traverse_stack(node, parent, add_dependencies_to_node)?;
                                }

                                Ok(())
                            };

                            traverse_stack(node, stack, add_dependencies_to_node).with_context(
                                |_| format_err!("Handling script, ID {}", request_id),
                            )?;
                        }
                    }
                };
            }
        }

        Ok(graph)
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

    pub fn duplicate_domains(&self) {
        let mut nodes_per_domain = HashMap::<_, Vec<_>>::new();
        for node in self.graph.node_indices() {
            let weight = self.graph.node_weight(node).unwrap();

            let mut entry = nodes_per_domain
                .entry(&weight.normalized_domain_name)
                .or_insert_with(Vec::new);
            entry.push(node);
        }
        let mut first = true;
        for (domain, nodes) in nodes_per_domain
            .into_iter()
            .filter(|(_, nodes)| nodes.len() > 1)
        {
            if first {
                info!("List of duplicate domains:");
                first = false;
            }
            info!("  Duplicate '{}' ({})", domain, nodes.len());
            for (i, node) in nodes.iter().enumerate() {
                let mut deps: Vec<_> = self.graph
                    .neighbors(*node)
                    .into_iter()
                    .map(|neigh| {
                        &self.graph
                            .node_weight(neigh)
                            .unwrap()
                            .normalized_domain_name
                    })
                    .collect();
                deps.sort();
                info!("    Dependency Set {} ({})", i + 1, deps.len());
                for dep in deps {
                    info!("      {}", dep);
                }
            }
        }
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

        let depgraph = DepGraph::new(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
            .expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
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

        let depgraph = DepGraph::new(&get_messages("./test/data/minimal-webpage-2018-05-08.json")
            .expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
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

        let mut depgraph = DepGraph::new(&get_messages(
            "./test/data/minimal-webpage-2018-05-08.json",
        ).expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
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

        let mut depgraph = DepGraph::new(&get_messages(
            "./test/data/minimal-webpage-2018-05-08.json",
        ).expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
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

        let mut depgraph = DepGraph::new(&get_messages(
            "./test/data/minimal-webpage-2018-05-08.json",
        ).expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
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

        let mut depgraph = DepGraph::new(&get_messages(
            "./test/data/minimal-webpage-2018-05-08.json",
        ).expect("Parsing the file must succeed."))
            .context(
            "Failed to process all messages from chrome",
        )
            .expect("A graph must be buildable from the data.");
        depgraph.simplify_graph();

        test_graphs_are_isomorph(&expected_graph, depgraph.as_graph());
    }
}
