use chrome::{ChromeDebuggerMessage, Initiator, RedirectResponse, Request, Script};
use failure::{Error, ResultExt};
use petgraph::{graph::NodeIndex, Directed, Graph};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use RequestInfo;

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
                // Add dependencies for node/msg combination
                match (initiator, redirect_response) {
                    (Initiator::Other {}, None) => {
                        let other = find_node("other".into())
                            .with_context(|_| format_err!("Handling other, ID {}", request_id))?;
                        add_dependency(node, other);
                    }
                    (Initiator::Other {}, Some(RedirectResponse { ref url })) => {
                        let other = find_node(url.clone())
                            .with_context(|_| format_err!("Handling redirect, ID {}", request_id))?;
                        add_dependency(node, other);
                    }
                    (Initiator::Parser { ref url }, None) => {
                        let other = find_node(url.clone())
                            .with_context(|_| format_err!("Handling parser, ID {}", request_id))?;
                        add_dependency(node, other);
                    }
                    (Initiator::Script { ref stack }, None) => {
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
                    _ => {
                        bail!("RedirectorResponse should only occur with the Initiator type other.")
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use misc_utils::fs::file_open_read;
    use petgraph::{
        algo::is_isomorphic,
        graphml::{Config as GraphMLConfig, GraphML},
    };
    use serde_json;
    use std::path::Path;

    fn build_from_file<P>(path: P) -> Result<DepGraph, Error>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let rdr = file_open_read(path)
            .map_err(|err| format_err!("Opening input file '{}' failed: {}", path.display(), err))?;
        let messages: Vec<ChromeDebuggerMessage> =
            serde_json::from_reader(rdr).context("Failed to parse JSON")?;
        let mut depgraph = DepGraph::new();
        depgraph
            .process_messages(&messages)
            .context("Failed to process all messages from chrome")?;
        Ok(depgraph)
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
            (localhost, other),
            (favicon, other),
            (localhost_script, localhost),
            (jquery, localhost),
            (fedora, localhost_script),
            (google, localhost),
            (pythonhaven, localhost),
            (pythonhaven, jquery),
        ]);

        let depgraph = build_from_file("./test/data/minimal-webpage-2018-05-08.json")
            .expect("Parsing the file must succeed.");

        assert_eq!(
            expected_graph.node_count(),
            depgraph.as_graph().node_count(),
            "Node count must be equal"
        );
        assert_eq!(
            expected_graph.edge_count(),
            depgraph.as_graph().edge_count(),
            "Edge count must be equal"
        );
        eprintln!(
            "{}",
            GraphML::with_config(
                &expected_graph,
                GraphMLConfig::new().export_node_weights(true)
            ).to_string_with_weight_functions(
                |_| Vec::new(),
                |node| vec![("weight".to_string(), node.to_string().into())],
            )
        );
        eprintln!(
            "{}",
            GraphML::with_config(
                depgraph.as_graph(),
                GraphMLConfig::new().export_node_weights(true)
            ).to_string_with_weight_functions(|_| Vec::new(), RequestInfo::graphml_support)
        );

        let pure_expected_graph = expected_graph.map(|_, _| (), |_, _| ());
        let pure_depgraph = depgraph.as_graph().map(|_, _| (), |_, _| ());
        assert!(
            is_isomorphic(&pure_expected_graph, &pure_depgraph),
            "Graphs must be isomorphic"
        );
    }
}
