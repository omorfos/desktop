//! What records from a microphone or a camera, or the sound that plays, and
//! what a screen is shared with through the portal, as `omorfos-recording`
//! finds it in PipeWire and tells the compositor.
//!
//! It is a program of its own, so that the compositor need not speak to
//! PipeWire, and it says what it finds over a pipe that only it and the
//! compositor have: a line for each thing recording, and a line with a
//! full stop when it has said everything. The mark that says a screen is
//! captured shows a sign for each kind, and its tooltip says which program
//! does what. See `docs/adr/0043-what-records.md`.

use std::io::{ErrorKind, Read};
use std::process::ChildStdout;
use std::time::Instant;

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use smithay::reexports::calloop::RegistrationToken;

use crate::capture::LINGER;
use crate::compositor::Compositor;

/// The most things that are said to be recording at once, and the longest
/// line saying one: what is said beyond them is not believed.
const MOST: usize = 64;
const LONGEST: usize = 256;

/// What something records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// From a microphone.
    Microphone,
    /// The sound that plays.
    Sound,
    /// From a camera.
    Camera,
    /// A screen, shared with it by another program, the portal.
    Screen,
}

/// A program recording, by the number of its process; and, for a screen
/// shared with it, the number of the process that shares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Recorder {
    pub(crate) kind: Recorded,
    pub(crate) pid: u32,
    pub(crate) through: Option<u32>,
}

impl Recorder {
    /// What one line says, if it says anything this knows.
    fn read(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        let kind = match words.next() {
            Some("microphone") => Recorded::Microphone,
            Some("sound") => Recorded::Sound,
            Some("camera") => Recorded::Camera,
            Some("screen") => Recorded::Screen,
            _ => return None,
        };
        let mut number = || words.next().and_then(|word| word.parse::<u32>().ok());
        let pid = number()?;
        let through = number().filter(|_| kind == Recorded::Screen);
        Some(Self { kind, pid, through })
    }
}

/// What has been heard of what records.
#[derive(Debug, Default)]
pub(crate) struct Recording {
    /// What is recording, as last said in full.
    pub(crate) now: Vec<Recorder>,
    /// What is being said, until it has all been.
    coming: Vec<Recorder>,
    /// What has been read of a line that has not ended.
    partial: Vec<u8>,
    /// Kinds that have stopped, which the mark shows until when.
    lingering: Vec<(Recorded, Instant)>,
    /// What the watcher says it through, and what wakes the compositor
    /// when it has.
    from: Option<(ChildStdout, RegistrationToken)>,
}

impl Recording {
    /// Takes in what the watcher said.
    fn heard(&mut self, bytes: &[u8], now: Instant) {
        for byte in bytes {
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.partial);
                self.line(&String::from_utf8_lossy(&line), now);
            } else if self.partial.len() < LONGEST {
                self.partial.push(*byte);
            }
        }
    }

    /// Takes in one line.
    fn line(&mut self, line: &str, now: Instant) {
        if line == "." {
            let said = std::mem::take(&mut self.coming);
            self.changed(said, now);
            return;
        }
        let recorder = Recorder::read(line).filter(|_| self.coming.len() < MOST);
        self.coming.extend(recorder);
    }

    /// Has what records be `said`: a kind that nothing records any more
    /// stays on the mark a moment, as a picture of the screen does.
    fn changed(&mut self, said: Vec<Recorder>, now: Instant) {
        let gone = self.now.iter().map(|recorder| recorder.kind);
        let gone: Vec<Recorded> = gone
            .filter(|kind| !said.iter().any(|recorder| recorder.kind == *kind))
            .collect();
        self.lingering
            .extend(gone.into_iter().map(|kind| (kind, now + LINGER)));
        self.now = said;
    }

    /// Whether anything of a kind records, or did a moment ago.
    pub(crate) fn any(&self, kinds: &[Recorded]) -> bool {
        let now = self.now.iter().map(|recorder| recorder.kind);
        let lingering = self.lingering.iter().map(|(kind, _)| *kind);
        now.chain(lingering).any(|kind| kinds.contains(&kind))
    }

    /// When the last kind that stopped goes from the mark.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.lingering.iter().map(|(_, until)| *until).min()
    }
}

impl Compositor {
    /// Listens to the watcher started as a process whose output is `from`.
    pub(crate) fn listen_to(&mut self, from: ChildStdout) {
        let flags = fcntl_getfl(&from).unwrap_or(OFlags::empty());
        let _ = fcntl_setfl(&from, flags | OFlags::NONBLOCK);
        let token = self.wake_on(&from);
        self.capture.recording.from = token.map(|token| (from, token));
    }

    /// Reads what the watcher has said since last time, and lets go of
    /// what has stopped long enough ago. A watcher that has stopped has
    /// nothing recording that it knows of.
    pub(crate) fn settle_recording(&mut self, now: Instant) {
        let mut bytes = [0; 4096];
        while let Some((mut from, token)) = self.capture.recording.from.take() {
            let read = from.read(&mut bytes);
            if read
                .as_ref()
                .is_err_and(|error| error.kind() == ErrorKind::WouldBlock)
            {
                self.capture.recording.from = Some((from, token));
                break;
            }
            let length = read.unwrap_or(0);
            if length == 0 {
                self.event_loop.remove(token);
                self.capture.recording.partial.clear();
                self.capture.recording.changed(Vec::new(), now);
                break;
            }
            self.capture.recording.heard(&bytes[..length], now);
            self.capture.recording.from = Some((from, token));
        }
        self.capture
            .recording
            .lingering
            .retain(|(_, until)| *until > now);
    }
}
