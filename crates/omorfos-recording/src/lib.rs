//! Tells the omorfos desktop what records from a microphone or a camera,
//! or records the sound that plays, and what a screen shared through the
//! portal is shown to: as PipeWire's graph has it.
//!
//! The compositor starts this, and reads what it says from it: a line for
//! each program recording, the kind of thing and the number of its
//! process, and a full stop when that is all, each time it changes. A
//! program's process is the one PipeWire found its connection comes from,
//! which the program cannot choose. The compositor shows what this says on
//! the mark that says something is captured. See
//! `docs/adr/0043-what-records.md`.
//!
//! A program that opens a camera itself, and not through PipeWire, is not
//! seen.

mod graph;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::error::Error;
use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

pub use graph::{Graph, Node};
use pipewire as pw;
use pw::registry::GlobalObject;
use pw::spa::utils::dict::DictRef;
use pw::types::ObjectType;

/// Why watching could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// How long the watcher waits for PipeWire to say something before it
/// looks at whether it is to stop.
const TURN: Duration = Duration::from_millis(100);

/// What is kept of what is followed: a node that records, to be told
/// whether it runs, and a client, to be told all that is known of it.
#[expect(
    dead_code,
    reason = "held so that PipeWire goes on telling of each, and let go with it"
)]
enum Followed {
    Node(pw::node::Node, pw::node::NodeListener),
    Client(pw::client::Client, pw::client::ClientListener),
}

/// What is followed, by its number in the graph.
type Bound = BTreeMap<u32, Followed>;

/// What the PipeWire layer that speaks PulseAudio's protocol says it is,
/// which is the way most programs record sound.
const PULSE: &str = "pipewire-pulse";

/// Watches the PipeWire at `remote`, or the one a program ordinarily
/// finds, and says on `out` what records whenever that changes: a line
/// each, as [`Graph::said`] has them, and then a full stop. It goes on
/// until `done` says to stop.
///
/// # Errors
///
/// Returns why it could not go on: there is no PipeWire, PipeWire went, or
/// what it says could not be written.
pub fn watch(
    remote: Option<&str>,
    out: &mut dyn Write,
    done: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    pw::init();
    let at = remote.map(|remote| pw::properties::properties! { *pw::keys::REMOTE_NAME => remote });
    let connected = pw::main_loop::MainLoopRc::new(None).and_then(|main| {
        pw::context::ContextRc::new(&main, None).and_then(|context| {
            context.connect_rc(at).and_then(|core| {
                let registry = core.get_registry_rc();
                registry.map(|registry| (main, context, core, registry))
            })
        })
    });
    let (main, _context, core, registry) = connected?;
    let graph = Rc::new(RefCell::new(Graph::default()));
    let bound = Rc::new(RefCell::new(Bound::new()));
    let lost = Rc::new(Cell::new(false));
    let losing = Rc::clone(&lost);
    // The number of PipeWire's own process, which the layer that speaks
    // PulseAudio's protocol is the program of as well.
    let daemon = Rc::new(Cell::new(None));
    let finding = Rc::clone(&daemon);
    let _core = core
        .add_listener_local()
        .info(move |info| {
            let pid = info
                .props()
                .and_then(|props| props.get(*pw::keys::APP_PROCESS_ID));
            finding.set(pid.and_then(|pid| pid.parse::<u32>().ok()));
        })
        .error(move |id, _, _, _| losing.set(losing.get() || id == pw::core::PW_ID_CORE))
        .register();
    let (noting, binding, binder) = (Rc::clone(&graph), Rc::clone(&bound), registry.clone());
    let noting = (noting, binding, Rc::clone(&daemon));
    let (forgetting, unbinding) = (Rc::clone(&graph), Rc::clone(&bound));
    let _registry = registry
        .add_listener_local()
        .global(move |global| note(&binder, &noting, global))
        .global_remove(move |id| {
            forgetting.borrow_mut().gone(id);
            unbinding.borrow_mut().remove(&id);
        })
        .register();
    let mut before = None;
    while !done() {
        main.loop_().iterate(pw::loop_::Timeout::Finite(TURN));
        if lost.get() {
            return Err("PipeWire has gone".into());
        }
        let said = graph.borrow().said();
        if before.as_ref() != Some(&said) {
            let mut report = String::new();
            for line in &said {
                report.push_str(line);
                report.push('\n');
            }
            report.push_str(".\n");
            out.write_all(report.as_bytes())
                .and_then(|()| out.flush())?;
            before = Some(said);
        }
    }
    Ok(())
}

/// What the graph is kept in, what is followed, and the number of
/// PipeWire's own process once it is known.
type Noting = (
    Rc<RefCell<Graph>>,
    Rc<RefCell<Bound>>,
    Rc<Cell<Option<u32>>>,
);

/// Notes what has come into the graph that says something records: a
/// node, which is followed to be told whether it runs if it is a stream
/// that records; a link; or a client, which is followed to be told the
/// number of its process.
fn note(registry: &pw::registry::RegistryRc, noting: &Noting, global: &GlobalObject<&DictRef>) {
    let (graph, bound, daemon) = noting;
    let said = |key: &str| global.props.and_then(|props| props.get(key));
    let number = |key: &str| said(key).and_then(|said| said.parse::<u32>().ok());
    let id = global.id;
    match global.type_ {
        ObjectType::Node => {
            let class = said(*pw::keys::MEDIA_CLASS).unwrap_or_default().to_owned();
            let records = class.starts_with("Stream/Input/");
            let node = Node {
                class,
                client: number(*pw::keys::CLIENT_ID),
                running: false,
                // A screen is offered as a source of pictures, as a camera
                // is, and says it is a screen.
                screen: said(*pw::keys::MEDIA_ROLE) == Some("Screen"),
            };
            graph.borrow_mut().node(id, node);
            let followed = records.then(|| registry.bind::<pw::node::Node, _>(global).ok());
            let followed = followed.flatten().map(|node| {
                let graph = Rc::clone(graph);
                let listener = node
                    .add_listener_local()
                    .info(move |info| {
                        let running = matches!(info.state(), pw::node::NodeState::Running);
                        graph.borrow_mut().running(id, running);
                    })
                    .register();
                Followed::Node(node, listener)
            });
            bound
                .borrow_mut()
                .extend(followed.map(|followed| (id, followed)));
        }
        ObjectType::Link => {
            let ends = number(*pw::keys::LINK_OUTPUT_NODE).zip(number(*pw::keys::LINK_INPUT_NODE));
            graph.borrow_mut().link(id, ends);
        }
        ObjectType::Client => {
            let followed = registry.bind::<pw::client::Client, _>(global).ok();
            let followed = followed.map(|client| {
                let (graph, daemon) = (Rc::clone(graph), Rc::clone(daemon));
                let listener = client
                    .add_listener_local()
                    .info(move |info| {
                        let pid = info.props().and_then(|props| pid(props, daemon.get()));
                        graph.borrow_mut().client(id, pid);
                    })
                    .register();
                Followed::Client(client, listener)
            });
            bound
                .borrow_mut()
                .extend(followed.map(|followed| (id, followed)));
        }
        _ => {}
    }
}

/// The number of the process a client is for: the one PipeWire found its
/// connection comes from, which a program cannot choose; or, for the layer
/// that speaks PulseAudio's protocol for the program, the one that layer
/// found the program's own connection comes from. That layer is believed
/// only if it runs what PipeWire itself runs, as the real one does, which
/// no other program can.
fn pid(props: &DictRef, daemon: Option<u32>) -> Option<u32> {
    let number = |key: &str| props.get(key).and_then(|said| said.parse::<u32>().ok());
    let connected = number(*pw::keys::SEC_PID);
    let layer = props.get(*pw::keys::CLIENT_API) == Some(PULSE) && same_program(connected, daemon);
    let passed = number(*pw::keys::APP_PROCESS_ID).filter(|_| layer);
    passed.or(connected)
}

/// Whether two processes run the same file.
fn same_program(one: Option<u32>, other: Option<u32>) -> bool {
    let file = |pid: u32| std::fs::read_link(format!("/proc/{pid}/exe")).ok();
    let one = one.and_then(file);
    one.is_some() && one == other.and_then(file)
}
