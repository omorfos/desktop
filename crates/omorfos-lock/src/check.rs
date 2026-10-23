//! The helper: asks PAM, through one service, whether the user is who
//! they say, and says what PAM asks and says on the way.
//!
//! The lock screen starts one for each way of unlocking, as `omorfos-lock
//! check` with the service and the user's name, so that PAM's modules, which
//! do not promise to be safe beside another transaction in one process,
//! each have a process of their own, and one that crashes or hangs takes
//! nothing else with it.
//!
//! It speaks the line protocol polkit's helper speaks: a word and what PAM
//! asked or said, a line each, until `SUCCESS`, `FAILURE`, or, if the way
//! cannot be asked here at all, `UNAVAILABLE`. What is typed is read back a
//! line for each question. This is the one file that touches PAM, through
//! `nonstick` (ADR 52).

use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, Write};

use nonstick::{AuthnFlags, ConversationAdapter, ErrorCode, Transaction, TransactionBuilder};

/// How a check ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checked {
    /// The user is who they say.
    Right,
    /// They are not, or gave up.
    Wrong,
    /// This way cannot be asked here: its module is not there, or the user
    /// has nothing enrolled for it.
    Unavailable,
}

impl Checked {
    /// The word the helper ends with.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Right => "SUCCESS",
            Self::Wrong => "FAILURE",
            Self::Unavailable => "UNAVAILABLE",
        }
    }
}

/// Asks PAM through `service` whether the user called `user` is who they
/// say, answering its questions with what is read from `heard` and saying
/// what it asks and says to `say`. Returns how it ended, which has been
/// said as well. With no user named, PAM asks who it is.
pub fn check(service: &OsStr, user: &OsStr, heard: impl BufRead, say: impl Write) -> Checked {
    let (heard, say) = (RefCell::new(heard), RefCell::new(say));
    let asked = Asked {
        heard: &heard,
        say: &say,
    };
    let builder = TransactionBuilder::new_with_service(service);
    let builder = if user.is_empty() {
        builder
    } else {
        builder.username(user)
    };
    let built = builder.build(asked.into_conversation());
    let checked = match built.map(|mut transaction| transaction.authenticate(AuthnFlags::empty())) {
        Ok(Ok(())) => Checked::Right,
        Ok(Err(why)) => checked(why),
        // A service that cannot even be begun is not set up here.
        Err(_) => Checked::Unavailable,
    };
    tell(&say, checked.word(), None);
    checked
}

/// What a failure to authenticate means for the way that was asked: that
/// the user was not who they said, or that the way cannot be asked here.
fn checked(why: ErrorCode) -> Checked {
    match why {
        ErrorCode::AuthenticationError
        | ErrorCode::MaxTries
        | ErrorCode::CredentialsInsufficient
        | ErrorCode::PermissionDenied
        | ErrorCode::ConversationError => Checked::Wrong,
        _ => Checked::Unavailable,
    }
}

/// Says `word` to `say` and, if there is one, what PAM said, on a line of
/// its own however many lines PAM wrote it in.
fn tell(say: &RefCell<impl Write>, word: &str, said: Option<&OsStr>) {
    let said = said.map(|said| said.to_string_lossy().replace(['\n', '\r'], " "));
    let mut say = say.borrow_mut();
    // If the lock screen has gone, there is no one to tell.
    let _ = match said {
        Some(said) => writeln!(say, "{word} {said}"),
        None => writeln!(say, "{word}"),
    };
    let _ = say.flush();
}

/// The conversation: what PAM asks and says, passed on a line each, and
/// what is typed, read back. PAM's questions for machines rather than
/// people are left to `nonstick`, which answers that they cannot be.
struct Asked<'a, R, W> {
    heard: &'a RefCell<R>,
    say: &'a RefCell<W>,
}

impl<R: BufRead, W: Write> Asked<'_, R, W> {
    /// Asks `question` as `word` says, and returns the line typed in
    /// answer, cut at the first NUL, which PAM cannot be handed.
    fn ask(&self, word: &str, question: &OsStr) -> nonstick::Result<OsString> {
        tell(self.say, word, Some(question));
        let mut typed = String::new();
        let read = self.heard.borrow_mut().read_line(&mut typed);
        // Nothing more to read is the lock screen gone, or giving up.
        read.ok()
            .filter(|read| *read > 0)
            .ok_or(ErrorCode::ConversationError)?;
        let typed = typed.trim_end_matches(['\n', '\r']);
        let typed = typed.split('\0').next().unwrap_or_default();
        Ok(OsString::from(typed))
    }
}

impl<R: BufRead, W: Write> ConversationAdapter for Asked<'_, R, W> {
    fn prompt(&self, request: impl AsRef<OsStr>) -> nonstick::Result<OsString> {
        self.ask("PAM_PROMPT_ECHO_ON", request.as_ref())
    }

    fn masked_prompt(&self, request: impl AsRef<OsStr>) -> nonstick::Result<OsString> {
        self.ask("PAM_PROMPT_ECHO_OFF", request.as_ref())
    }

    fn error_msg(&self, message: impl AsRef<OsStr>) {
        tell(self.say, "PAM_ERROR_MSG", Some(message.as_ref()));
    }

    fn info_msg(&self, message: impl AsRef<OsStr>) {
        tell(self.say, "PAM_TEXT_INFO", Some(message.as_ref()));
    }
}
