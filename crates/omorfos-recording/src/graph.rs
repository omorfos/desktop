//! What is in PipeWire's graph that says something records, and what it
//! comes to: a line for each program recording, and what from.

use std::collections::BTreeMap;

/// A node of the graph: what kind of thing it is, the client that made it,
/// and whether it is running, which a stream that records is while it
/// does.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// What PipeWire calls its class, such as `Stream/Input/Audio`.
    pub class: String,
    /// The client that made it.
    pub client: Option<u32>,
    /// Whether it is running.
    pub running: bool,
    /// Whether it says it is a screen, which a source of pictures shared
    /// through the portal does.
    pub screen: bool,
}

/// What is known of the graph.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    nodes: BTreeMap<u32, Node>,
    /// Each link, from the node it goes out of to the node it goes into.
    links: BTreeMap<u32, (u32, u32)>,
    /// The number of each client's process, as PipeWire found it from the
    /// client's connection, which the client cannot choose.
    clients: BTreeMap<u32, u32>,
}

impl Graph {
    /// Notes a node, or what it is now.
    pub fn node(&mut self, id: u32, node: Node) {
        self.nodes.insert(id, node);
    }

    /// Notes whether a node is running.
    pub fn running(&mut self, id: u32, running: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.running = running;
        }
    }

    /// Notes a link from one node into another, by its `ends`. One whose
    /// ends are not known is no link.
    pub fn link(&mut self, id: u32, ends: Option<(u32, u32)>) {
        if let Some(ends) = ends {
            self.links.insert(id, ends);
        }
    }

    /// Notes a client and the number of its process. One whose process is
    /// not known records nothing anybody can be told of.
    pub fn client(&mut self, id: u32, pid: Option<u32>) {
        if let Some(pid) = pid {
            self.clients.insert(id, pid);
        }
    }

    /// Forgets whatever is known by `id`, which has gone.
    pub fn gone(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.links.remove(&id);
        self.clients.remove(&id);
    }

    /// The number of the process that made a node.
    fn pid(&self, node: &Node) -> Option<u32> {
        node.client
            .and_then(|client| self.clients.get(&client).copied())
    }

    /// What records, a line each, in order and each once: `microphone`,
    /// `sound` or `camera` and the number of the process; or `screen`, the
    /// number of the process shown a screen and that of the one sharing
    /// it. A stream records while it runs, from what is linked into it.
    #[must_use]
    pub fn said(&self) -> Vec<String> {
        let mut said = Vec::new();
        let recording = self.nodes.iter().filter(|(_, node)| node.running);
        for (id, node) in recording {
            let Some(pid) = self.pid(node) else { continue };
            let from = self.links.values().filter(|(_, into)| into == id);
            let from = from.filter_map(|(from, _)| self.nodes.get(from));
            for source in from {
                let line = match (node.class.as_str(), source.class.as_str()) {
                    ("Stream/Input/Audio", "Audio/Source") => format!("microphone {pid}"),
                    ("Stream/Input/Audio", _) => format!("sound {pid}"),
                    ("Stream/Input/Video", _) if source.screen => {
                        let Some(through) = self.pid(source) else {
                            continue;
                        };
                        format!("screen {pid} {through}")
                    }
                    ("Stream/Input/Video", "Video/Source") => format!("camera {pid}"),
                    _ => continue,
                };
                said.push(line);
            }
        }
        said.sort();
        said.dedup();
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(class: &str, client: u32, running: bool) -> Node {
        Node {
            class: class.to_owned(),
            client: Some(client),
            running,
            screen: false,
        }
    }

    /// A graph with a microphone, a camera, the sound that plays, and a
    /// screen shared by the portal, each linked into a stream of its own
    /// of the program numbered 7, whose client is 70; the portal's is 80,
    /// its process 8.
    fn graph() -> Graph {
        let mut graph = Graph::default();
        graph.client(70, Some(7));
        graph.client(80, Some(8));
        graph.client(90, Some(9));
        graph.client(60, None);
        graph.node(1, node("Audio/Source", 90, true));
        graph.node(2, node("Audio/Sink", 90, true));
        graph.node(3, node("Video/Source", 90, true));
        // The portal's screen is a source of pictures, as a camera is.
        let screen = node("Video/Source", 80, true);
        graph.node(
            4,
            Node {
                screen: true,
                ..screen
            },
        );
        for (id, class) in [
            (11, "Stream/Input/Audio"),
            (12, "Stream/Input/Audio"),
            (13, "Stream/Input/Video"),
            (14, "Stream/Input/Video"),
        ] {
            graph.node(id, node(class, 70, true));
        }
        for (link, from, into) in [(21, 1, 11), (22, 2, 12), (23, 3, 13), (24, 4, 14)] {
            graph.link(link, Some((from, into)));
        }
        graph.link(25, None);
        graph
    }

    #[test]
    fn what_is_linked_into_a_running_stream_says_what_it_records() {
        let said = ["camera 7", "microphone 7", "screen 7 8", "sound 7"];
        assert_eq!(graph().said(), said);
    }

    #[test]
    fn a_stream_that_is_not_running_or_is_not_known_records_nothing() {
        let mut graph = graph();
        graph.running(11, false);
        graph.running(99, false);
        graph.gone(22);
        graph.gone(80);
        assert_eq!(
            graph.said(),
            ["camera 7"],
            "nor a screen of an unknown sharer"
        );

        // Nor does one whose process is not known, or what is not a
        // stream that records.
        let mut graph = self::graph();
        graph.gone(70);
        assert_eq!(graph.said(), [""; 0]);
        let mut graph = self::graph();
        graph.node(15, node("Stream/Output/Audio", 70, true));
        graph.link(25, Some((1, 15)));
        graph.node(16, node("Stream/Input/Video", 70, true));
        graph.link(26, Some((1, 16)));
        let unclaimed = Node::default();
        graph.node(
            17,
            Node {
                running: true,
                ..unclaimed
            },
        );
        graph.link(27, Some((1, 17)));
        assert_eq!(graph.said().len(), 4, "the four there were, and no more");
    }
}
