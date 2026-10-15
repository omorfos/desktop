//! The watcher against a PipeWire of its own, with streams that stand for
//! a microphone, the sound that plays, a camera and a shared screen, each
//! recorded from by a stream of this process.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::pod::Pod;
use pw::spa::pod::serialize::PodSerializer;

/// A PipeWire of its own, in a folder of its own, for as long as it lasts.
struct Pipewire {
    child: Child,
    folder: tempfile::TempDir,
}

impl Pipewire {
    fn start() -> Self {
        let folder = tempfile::Builder::new()
            .prefix("pw")
            .tempdir_in("/tmp")
            .unwrap();
        let child = Command::new("pipewire")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("XDG_RUNTIME_DIR", folder.path())
            .env("PIPEWIRE_RUNTIME_DIR", folder.path())
            .env("HOME", folder.path())
            .env("PIPEWIRE_LOG_SYSTEMD", "false")
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let socket = folder.path().join("pipewire-0");
        for _ in 0..500 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Self { child, folder }
    }

    fn socket(&self) -> PathBuf {
        self.folder.path().join("pipewire-0")
    }
}

impl Drop for Pipewire {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Hands on each whole report the watcher writes: its lines, without the
/// full stop.
struct Reports {
    line: Vec<u8>,
    report: Vec<String>,
    to: Sender<Vec<String>>,
}

impl Write for Reports {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for byte in bytes {
            if *byte == b'\n' {
                let line = String::from_utf8(std::mem::take(&mut self.line)).unwrap();
                if line == "." {
                    let _ = self.to.send(std::mem::take(&mut self.report));
                } else {
                    self.report.push(line);
                }
            } else {
                self.line.push(*byte);
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Watches the PipeWire at `socket` on a thread of its own, until told to
/// stop; and says what it saw end it.
fn watching(
    socket: PathBuf,
    stop: Arc<AtomicBool>,
) -> (Receiver<Vec<String>>, std::thread::JoinHandle<String>) {
    let (to, from) = channel();
    let watcher = std::thread::spawn(move || {
        let mut reports = Reports {
            line: Vec::new(),
            report: Vec::new(),
            to,
        };
        let done = || stop.load(Ordering::Relaxed);
        let remote = socket.to_str().unwrap().to_owned();
        let watched = omorfos_recording::watch(Some(&remote), &mut reports, &done);
        watched.map_or_else(|failure| failure.to_string(), |()| "stopped".to_owned())
    });
    (from, watcher)
}

/// The next report that `wanted` picks out, waiting a while for it.
fn report(from: &Receiver<Vec<String>>, wanted: impl Fn(&[String]) -> bool) -> Vec<String> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let left = until.saturating_duration_since(Instant::now());
        let report = from.recv_timeout(left).unwrap();
        if wanted(&report) {
            return report;
        }
    }
}

/// What a stream offers and takes: small pictures, which every stream can
/// be linked for at once, as sound cannot until it has been converted.
fn pictures() -> Vec<u8> {
    use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    let wants = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
        spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Id,
            spa::param::video::VideoFormat::BGRx
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Rectangle,
            spa::utils::Rectangle {
                width: 16,
                height: 16
            }
        ),
    );
    let value = spa::pod::Value::Object(wants);
    PodSerializer::serialize(Cursor::new(Vec::new()), &value)
        .unwrap()
        .0
        .into_inner()
}

/// Streams that stand for what is recorded from, each linked into one of
/// this process that records it, on a thread of their own until told to
/// stop.
fn recorded(socket: PathBuf, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        pw::init();
        let main = pw::main_loop::MainLoopRc::new(None).unwrap();
        let context = pw::context::ContextRc::new(&main, None).unwrap();
        let remote = socket.to_str().unwrap();
        let core = context
            .connect_rc(Some(properties! { *pw::keys::REMOTE_NAME => remote }))
            .unwrap();
        // A connection that says it is the layer that speaks PulseAudio's
        // protocol, for another process: it is not believed, as it does not
        // run what PipeWire runs.
        let claiming = properties! {
            *pw::keys::REMOTE_NAME => remote,
            *pw::keys::CLIENT_API => "pipewire-pulse",
            *pw::keys::APP_PROCESS_ID => "1",
        };
        let claimed = context.connect_rc(Some(claiming)).unwrap();
        // The portal's screen is a source of pictures, as a camera is, that
        // says it is a screen.
        let pairs = [
            ("Audio/Source", "Stream/Input/Audio", None, &core),
            ("Audio/Sink", "Stream/Input/Audio", None, &claimed),
            ("Video/Source", "Stream/Input/Video", None, &core),
            ("Video/Source", "Stream/Input/Video", Some("Screen"), &core),
        ];
        let mut streams = Vec::new();
        for (source, recorder, role, recording) in pairs {
            let ends = [
                (source, spa::utils::Direction::Output, role, &core),
                (recorder, spa::utils::Direction::Input, None, recording),
            ];
            for (class, direction, role, through) in ends {
                // Whatever they stand for, they are streams of pictures: only
                // what they are called matters to the watcher.
                let mut kind = properties! {
                    *pw::keys::MEDIA_TYPE => "Video",
                    *pw::keys::MEDIA_CLASS => class,
                };
                if let Some(role) = role {
                    kind.insert(*pw::keys::MEDIA_ROLE, role);
                }
                let stream = pw::stream::StreamRc::new((*through).clone(), class, kind).unwrap();
                let ready = std::rc::Rc::new(std::cell::Cell::new(false));
                let readying = std::rc::Rc::clone(&ready);
                let listener = stream
                    .add_local_listener::<()>()
                    .state_changed(move |_, (), _, now| {
                        if matches!(now, pw::stream::StreamState::Paused) {
                            readying.set(true);
                        }
                    })
                    .process(|stream, ()| {
                        // What comes is let go, and what goes is nothing.
                        let _ = stream.dequeue_buffer();
                    })
                    .register()
                    .unwrap();
                let wants = pictures();
                let mut wants = [Pod::from_bytes(&wants).unwrap()];
                // What is recorded from drives the graph, as a device does.
                let flags = if direction == spa::utils::Direction::Output {
                    pw::stream::StreamFlags::DRIVER | pw::stream::StreamFlags::ALLOC_BUFFERS
                } else {
                    pw::stream::StreamFlags::MAP_BUFFERS
                };
                stream.connect(direction, None, flags, &mut wants).unwrap();
                streams.push((stream, listener, ready));
            }
        }
        let mut links = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            main.loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
            for (stream, ..) in streams.iter().step_by(2) {
                let _ = stream.trigger_process();
            }
            let named: Vec<u32> = streams
                .iter()
                .map(|(stream, ..)| stream.node_id())
                .collect();
            let known = streams.iter().all(|(.., ready)| ready.get());
            if links.is_empty() && known {
                for pair in named.chunks(2) {
                    let ends = properties! {
                        "link.output.node" => pair[0].to_string(),
                        "link.input.node" => pair[1].to_string(),
                        "object.linger" => "false",
                    };
                    let link = core.create_object::<pw::link::Link>("link-factory", &ends);
                    links.push(link.unwrap());
                }
            }
        }
        drop(links);
        for (stream, ..) in &streams {
            let _ = stream.disconnect();
        }
        for _ in 0..20 {
            main.loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
        }
    })
}

#[test]
fn what_records_is_said_and_said_again_when_it_stops() {
    let mut pipewire = Pipewire::start();
    let socket = pipewire.socket();
    let finished = Arc::new(AtomicBool::new(false));
    let (reports, watcher) = watching(socket.clone(), Arc::clone(&finished));
    assert_eq!(
        report(&reports, |_| true),
        [""; 0],
        "nothing records at first"
    );

    let recording = Arc::new(AtomicBool::new(false));
    let streams = recorded(socket, Arc::clone(&recording));
    let me = std::process::id();
    let said = [
        format!("camera {me}"),
        format!("microphone {me}"),
        format!("screen {me} {me}"),
        format!("sound {me}"),
    ];
    assert_eq!(report(&reports, |report| report.len() == 4), said);

    // When they stop, nothing records.
    recording.store(true, Ordering::Relaxed);
    streams.join().unwrap();
    assert_eq!(report(&reports, <[String]>::is_empty), [""; 0]);

    // And when PipeWire goes, there is no more to watch.
    let _ = pipewire.child.kill();
    let _ = pipewire.child.wait();
    let ended = watcher.join().unwrap();
    assert_eq!(ended, "PipeWire has gone");
    finished.store(true, Ordering::Relaxed);
}

/// What cannot be written to.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn it_watches_until_told_to_stop_and_says_where_there_is_no_pipewire() {
    let pipewire = Pipewire::start();
    let stop = Arc::new(AtomicBool::new(false));
    let (reports, watcher) = watching(pipewire.socket(), Arc::clone(&stop));
    report(&reports, |_| true);
    stop.store(true, Ordering::Relaxed);
    assert_eq!(watcher.join().unwrap(), "stopped");

    // Nor once what it says cannot be written.
    let remote = pipewire.socket().to_str().unwrap().to_owned();
    let unheard = omorfos_recording::watch(Some(&remote), &mut Broken, &|| false);
    assert!(unheard.is_err());

    let nowhere = pipewire.socket().with_file_name("nowhere-0");
    let (_, watcher) = watching(nowhere, stop);
    assert_ne!(watcher.join().unwrap(), "stopped");
}
