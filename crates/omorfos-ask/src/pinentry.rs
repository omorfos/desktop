//! What a pinentry says to `gpg-agent`, which starts it to ask the user
//! for a passphrase, and what it asks of the sealed prompt.
//!
//! Nothing `gpg-agent` is given to say reaches the prompt (ADR 58): not the
//! description, the title or the error it sets. The prompt asks in words
//! of its own, of the key `gpg-agent` names by its keygrip, which is
//! looked up with GnuPG; the description is passed on to be kept in the
//! log, and nothing more.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::BorrowedFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};

use rustix::event::{PollFd, PollFlags, poll};

use crate::assuan::{self, escaped, unescaped};

/// What the prompt is asked to ask.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wanted {
    /// The key the passphrase is of, as GnuPG lists it, if it does.
    pub key: Option<Key>,
    /// Which passphrase: of the key, or a new one, first or again.
    pub step: Step,
    /// Whether the last one given was not right.
    pub again: bool,
    /// What `gpg-agent` was given to say, for the log alone.
    pub said: String,
}

/// Which passphrase is asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Step {
    /// The passphrase a key has.
    #[default]
    Passphrase,
    /// A new passphrase for it.
    New,
    /// The new passphrase again, to be sure of it.
    NewAgain,
}

/// A key, as GnuPG lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// Its first user ID, which is its owner's own words; or what kind of
    /// key it is, for one GnuPG holds for SSH.
    pub name: String,
    /// Its fingerprint, as GnuPG writes it.
    pub fingerprint: String,
}

/// The longest user ID shown, in characters: more is cut off.
const LONGEST_NAME: usize = 80;

/// What a pinentry says when it is cancelled, by the numbers GnuPG gives
/// its errors: the pinentry's own source and `GPG_ERR_CANCELED`.
const CANCELLED: &str = "ERR 83886179 Operation cancelled <Pinentry>";

/// What it says when there is nothing to ask with: `GPG_ERR_NO_PIN_ENTRY`.
const NO_PROMPT: &str = "ERR 83886165 No sealed prompt to ask in <Pinentry>";

/// What it says of a command it does not know: `GPG_ERR_ASS_UNKNOWN_CMD`.
const UNKNOWN: &str = "ERR 83886355 Unknown IPC command <Pinentry>";

/// What it says of what it does not do: `GPG_ERR_NOT_SUPPORTED`.
const NOT_DONE: &str = "ERR 83886140 Not supported <Pinentry>";

/// What a pinentry has been told, for the next passphrase it asks for.
#[derive(Debug, Default)]
pub struct Told {
    keyinfo: Option<String>,
    repeat: bool,
    error: bool,
    said: String,
}

/// What came of asking the user.
#[derive(Debug, PartialEq, Eq)]
pub enum Given {
    /// What they typed.
    Typed(String),
    /// They gave up.
    Cancelled,
    /// There is nothing to ask with.
    Nowhere,
}

/// What a line from `gpg-agent` comes to: what to say back, and whether
/// that is the last.
pub type Replies = (Vec<String>, bool);

/// What to say back to the line `line` from `gpg-agent`, having been told
/// what `told` holds; `ask` asks the user, and `lookup` finds the key a
/// keygrip is of.
pub fn reply(
    told: &mut Told,
    line: &str,
    ask: &mut dyn FnMut(&Wanted) -> Given,
    lookup: &dyn Fn(&str) -> Option<Key>,
) -> Replies {
    let ok = || vec!["OK".to_owned()];
    let (command, argument) = assuan::command(line);
    let replies = match command.as_str() {
        "BYE" => return (ok(), true),
        "SETKEYINFO" => {
            told.keyinfo = (argument != "--clear").then_some(argument);
            ok()
        }
        "SETDESC" => {
            told.said = argument;
            ok()
        }
        "SETERROR" => {
            told.error = !argument.is_empty();
            ok()
        }
        "SETREPEAT" => {
            told.repeat = true;
            ok()
        }
        "RESET" => {
            *told = Told::default();
            ok()
        }
        "GETPIN" => getpin(told, ask, lookup),
        // Asked to confirm, with words that are a program's, it says no
        // (ADR 58).
        "CONFIRM" if argument != "--one-button" => vec![NOT_DONE.to_owned()],
        "GETINFO" => getinfo(&argument),
        // What is to be said, and how, is ours: what it is told of it is
        // taken and left; and a message, which asks nothing, is said to
        // have been shown, as nothing need be done of it.
        "CONFIRM" | "MESSAGE" | "OPTION" | "SETPROMPT" | "SETTITLE" | "SETOK" | "SETCANCEL"
        | "SETNOTOK" | "SETREPEATERROR" | "SETREPEATOK" | "SETQUALITYBAR" | "SETQUALITYBAR_TT"
        | "SETGENPIN" | "SETGENPIN_TT" | "SETTIMEOUT" | "NOP" => ok(),
        _ => vec![UNKNOWN.to_owned()],
    };
    (replies, false)
}

/// Speaks to `gpg-agent` over `input` and `output` until it says it is
/// done or goes, asking at `door`, where `omorfos-ask` listens. `agent` is
/// what `input` is read from, watched while the user is asked, so that a
/// question `gpg-agent` will never hear the answer to is given up.
pub fn serve(
    input: impl BufRead,
    mut output: impl Write,
    (door, agent): (Option<&Path>, Option<BorrowedFd<'_>>),
) {
    let mut said = |lines: &[String]| {
        let all = lines
            .iter()
            .fold(String::new(), |all, line| all + line + "\n");
        output
            .write_all(all.as_bytes())
            .and_then(|()| output.flush())
            .is_ok()
    };
    let mut told = Told::default();
    let mut ask = |wanted: &Wanted| asked_at((door, agent), wanted);
    let mut going = said(&["OK Pleased to meet you".to_owned()]);
    for line in input.lines().map_while(Result::ok) {
        if !going {
            break;
        }
        let (replies, last) = reply(&mut told, &line, &mut ask, &lookup);
        going = said(&replies) && !last;
    }
}

/// Asks `omorfos-ask`, listening at `door`, to ask what is `wanted`, and
/// says what came of it: nothing if it is not there to ask, or if `agent`,
/// which asked, has gone meanwhile, as it goes when it stops; the
/// connection then ends, and the question with it.
#[must_use]
pub fn asked_at((door, agent): (Option<&Path>, Option<BorrowedFd<'_>>), wanted: &Wanted) -> Given {
    let connected = door.and_then(|door| UnixStream::connect(door).ok());
    let sent = connected.filter(|mut stream| stream.write_all(request(wanted).as_bytes()).is_ok());
    let answered = sent.filter(|stream| heard_first(stream, agent));
    let mut answer = String::new();
    let read = answered.is_some_and(|stream| BufReader::new(stream).read_line(&mut answer).is_ok());
    let given = match answer.strip_prefix("D ") {
        Some(typed) if read => Given::Typed(unescaped(typed.trim_end_matches('\n'))),
        _ if read && answer == "CANCEL\n" => Given::Cancelled,
        _ => Given::Nowhere,
    };
    answer.into_bytes().fill(0);
    given
}

/// Waits for `stream` to answer, or for `agent` to go: whether it answered
/// first.
fn heard_first(stream: &UnixStream, agent: Option<BorrowedFd<'_>>) -> bool {
    let mut waiting = vec![PollFd::new(stream, PollFlags::IN)];
    // Nothing is asked of it but that it goes, which is said regardless.
    waiting.extend(agent.map(|agent| PollFd::from_borrowed_fd(agent, PollFlags::empty())));
    let _ = poll(&mut waiting, None);
    waiting
        .get(1)
        .is_none_or(|agent| agent.revents().is_empty())
}

/// What a pinentry says of itself.
fn getinfo(what: &str) -> Vec<String> {
    let said = match what {
        "version" => Some(env!("CARGO_PKG_VERSION").to_owned()),
        "pid" => Some(std::process::id().to_string()),
        "flavor" => Some("omorfos".to_owned()),
        _ => None,
    };
    let data = said.map(|said| format!("D {}", escaped(&said)));
    data.into_iter().chain(["OK".to_owned()]).collect()
}

/// Asks for a passphrase, twice if it is a new one, and says what was
/// given.
fn getpin(
    told: &mut Told,
    ask: &mut dyn FnMut(&Wanted) -> Given,
    lookup: &dyn Fn(&str) -> Option<Key>,
) -> Vec<String> {
    // A keygrip comes after what kind of cache it is kept in, as `n/`.
    let grip = told
        .keyinfo
        .as_deref()
        .map(|info| info.rsplit('/').next().unwrap_or(info));
    let mut wanted = Wanted {
        key: grip.and_then(lookup),
        step: if told.repeat {
            Step::New
        } else {
            Step::Passphrase
        },
        again: std::mem::take(&mut told.error),
        said: std::mem::take(&mut told.said),
    };
    let first = ask(&wanted);
    let given = match first {
        Given::Typed(first) if told.repeat => {
            wanted.step = Step::NewAgain;
            wanted.again = false;
            match ask(&wanted) {
                Given::Typed(second) if second == first => Given::Typed(first),
                // Not the same: GnuPG is told nothing was given, and asks
                // again as it does for a cancelled one.
                Given::Typed(_) => Given::Cancelled,
                other => other,
            }
        }
        other => other,
    };
    told.repeat = false;
    match given {
        Given::Typed(typed) => {
            let data = format!("D {}", escaped(&typed));
            // What was typed is not kept here once it is on its way.
            typed.into_bytes().fill(0);
            let repeated = told_repeated(wanted.step);
            [data]
                .into_iter()
                .chain(repeated)
                .chain(["OK".to_owned()])
                .collect()
        }
        Given::Cancelled => vec![CANCELLED.to_owned()],
        Given::Nowhere => vec![NO_PROMPT.to_owned()],
    }
}

/// What tells `gpg-agent` that a new passphrase was asked for twice and
/// was the same both times, so that it does not ask again itself.
fn told_repeated(step: Step) -> Option<String> {
    (step == Step::NewAgain).then(|| "S PIN_REPEATED".to_owned())
}

/// The key a keygrip is of, among those GnuPG lists in `listed`, as
/// `gpg --with-colons --with-keygrip --list-secret-keys` lists them: its
/// first user ID and the fingerprint of the key it is part of.
#[must_use]
pub fn listed_key(listed: &str, grip: &str) -> Option<Key> {
    // Each key is a primary key and its subkeys, each with its fingerprint
    // and keygrip, and the user IDs that name them all.
    let mut found = None;
    let (mut fingerprint, mut name) = (None, None);
    let mut wanted = false;
    for line in listed.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        let field = |at: usize| fields.get(at).copied().unwrap_or_default();
        match field(0) {
            "sec" => {
                if wanted {
                    break;
                }
                (fingerprint, name) = (None, None);
            }
            "fpr" if fingerprint.is_none() => fingerprint = Some(field(9).to_owned()),
            "grp" => wanted |= field(9).eq_ignore_ascii_case(grip),
            "uid" if name.is_none() => name = Some(unescaped_colons(field(9))),
            _ => {}
        }
        if wanted {
            found = fingerprint.clone().zip(name.clone());
        }
    }
    found.map(|(fingerprint, name)| Key {
        name: tidied(&name),
        fingerprint: grouped(&fingerprint),
    })
}

/// What GnuPG writes in a field as `\x3a` and the like, read back.
fn unescaped_colons(text: &str) -> String {
    unescaped(&text.replace("\\x", "%"))
}

/// A user ID as it is shown: without what could not be seen, and cut off
/// where it is too long.
fn tidied(name: &str) -> String {
    let seen = name.chars().filter(|character| !character.is_control());
    let mut tidied: String = seen.take(LONGEST_NAME).collect();
    if name.chars().count() > LONGEST_NAME {
        tidied.push('\u{2026}');
    }
    tidied
}

/// A fingerprint in groups of four, as GnuPG shows one to people.
fn grouped(fingerprint: &str) -> String {
    let characters: Vec<char> = fingerprint.chars().collect();
    let groups = characters
        .chunks(4)
        .map(|group| group.iter().collect::<String>());
    groups.collect::<Vec<_>>().join(" ")
}

/// The SSH key a keygrip is of, as `gpg-agent` says of it in `said`, the
/// answer to `KEYINFO --ssh-fpr`.
#[must_use]
pub fn ssh_key(said: &str) -> Option<Key> {
    let line = said.lines().find(|line| line.starts_with("S KEYINFO "))?;
    let fingerprint = line
        .split(' ')
        .find(|word| word.starts_with("SHA256:") || word.starts_with("MD5:"))?;
    Some(Key {
        name: "An SSH key".to_owned(),
        fingerprint: fingerprint.to_owned(),
    })
}

/// The key a keygrip is of, as GnuPG says: among its secret keys, and
/// otherwise among those `gpg-agent` holds for SSH. GnuPG is asked in the
/// home the environment gives, which is the agent's that started this.
#[must_use]
pub fn lookup(grip: &str) -> Option<Key> {
    let run = |program: &str, arguments: &[&str]| {
        let output = Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        output
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let list = [
        "--batch",
        "--with-colons",
        "--with-keygrip",
        "--list-secret-keys",
    ];
    let listed = run("gpg", &list);
    let found = listed.and_then(|listed| listed_key(&listed, grip));
    found.or_else(|| {
        let asked = format!("KEYINFO --ssh-fpr {grip}");
        let agent = ["--no-autostart", &asked, "/bye"];
        run("gpg-connect-agent", &agent).and_then(|said| ssh_key(&said))
    })
}

/// A request to the prompt, as one line: which passphrase, whether the
/// last was not right, and the key and what `gpg-agent` was given to say,
/// each written so that it has no space or line's end in it.
#[must_use]
pub fn request(wanted: &Wanted) -> String {
    let step = match wanted.step {
        Step::Passphrase => "passphrase",
        Step::New => "new",
        Step::NewAgain => "again",
    };
    let word = |text: &str| format!("+{}", escaped(text).replace(' ', "%20"));
    let key = wanted.key.as_ref();
    let (name, fingerprint) = key.map_or_else(
        || ("-".to_owned(), "-".to_owned()),
        |key| (word(&key.name), word(&key.fingerprint)),
    );
    let again = if wanted.again { "wrong" } else { "first" };
    format!(
        "ASK {step} {again} {fingerprint} {name} {}\n",
        word(&wanted.said)
    )
}

/// What a line from the pinentry asks, if it is a request as [`request`]
/// writes one.
#[must_use]
pub fn requested(line: &str) -> Option<Wanted> {
    let mut words = line.trim_end().split(' ');
    let word = |word: Option<&str>| word.and_then(|word| word.strip_prefix('+')).map(unescaped);
    if words.next() != Some("ASK") {
        return None;
    }
    let step = match words.next()? {
        "passphrase" => Step::Passphrase,
        "new" => Step::New,
        "again" => Step::NewAgain,
        _ => return None,
    };
    let again = words.next()? == "wrong";
    let (fingerprint, name) = (word(words.next()), word(words.next()));
    let said = word(words.next()).unwrap_or_default();
    let key = fingerprint.zip(name).map(|(fingerprint, name)| Key {
        name: tidied(&name),
        fingerprint,
    });
    Some(Wanted {
        key,
        step,
        again,
        said,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What GnuPG lists of a key of Alice's, with a subkey to encrypt to.
    const LISTED: &str = "sec:u:255:22:AAAA1111BBBB2222:1700000000:::u:::scESC:::+:::23::0:
fpr:::::::::ABCD1234ABCD1234ABCD1234ABCD1234ABCD1234:
grp:::::::::1111111111111111111111111111111111111111:
uid:u::::1700000000::HASH::Alice Example <alice\\x3aexample.org>::::::::::0:
uid:u::::1700000000::HASH::Alice at work <alice@work.example>::::::::::0:
ssb:u:255:18:CCCC3333DDDD4444:1700000000::::::e:::+:::23:
fpr:::::::::EEEE5555EEEE5555EEEE5555EEEE5555EEEE5555:
grp:::::::::2222222222222222222222222222222222222222:
sec:u:255:22:FFFF6666FFFF6666:1700000000:::u:::scESC:::+:::23::0:
fpr:::::::::9999888899998888999988889999888899998888:
grp:::::::::3333333333333333333333333333333333333333:
uid:u::::1700000000::HASH::Bob <bob@example.org>::::::::::0:
";

    fn alice() -> Key {
        Key {
            name: "Alice Example <alice:example.org>".to_owned(),
            fingerprint: "ABCD 1234 ABCD 1234 ABCD 1234 ABCD 1234 ABCD 1234".to_owned(),
        }
    }

    #[test]
    fn a_keygrip_is_of_the_key_it_is_part_of_named_by_its_first_user_id() {
        let grip = "1111111111111111111111111111111111111111";
        assert_eq!(listed_key(LISTED, grip), Some(alice()));
        let subkey = "2222222222222222222222222222222222222222";
        assert_eq!(listed_key(LISTED, subkey), Some(alice()), "a subkey's too");
        let bob = listed_key(LISTED, "3333333333333333333333333333333333333333").unwrap();
        assert_eq!(bob.name, "Bob <bob@example.org>");
        assert_eq!(listed_key(LISTED, "4444"), None, "one it does not have");
        assert_eq!(listed_key("", "1111"), None);
    }

    #[test]
    fn a_user_id_is_shown_without_what_cannot_be_seen_and_not_too_long() {
        assert_eq!(tidied("Eve\u{7}\u{1b}[31m"), "Eve[31m");
        let long = "a".repeat(100);
        let shown = tidied(&long);
        assert_eq!(shown.chars().count(), LONGEST_NAME + 1);
        assert!(shown.ends_with('\u{2026}'));
    }

    #[test]
    fn an_ssh_key_is_known_by_its_fingerprint_as_the_agent_says() {
        let said = "S KEYINFO 5555 D - - - P SHA256:abcDEF123 - -\nOK\n";
        let key = ssh_key(said).unwrap();
        assert_eq!(key.fingerprint, "SHA256:abcDEF123");
        assert_eq!(key.name, "An SSH key");
        assert_eq!(ssh_key("OK\n"), None);
        assert_eq!(ssh_key("S KEYINFO 5555 D - - - P - - -\n"), None);
    }

    #[test]
    fn a_request_is_one_line_and_is_read_back_as_it_was() {
        let wanted = Wanted {
            key: Some(alice()),
            step: Step::NewAgain,
            again: true,
            said: "Please enter\nthe passphrase, 100%".to_owned(),
        };
        let line = request(&wanted);
        assert_eq!(line.matches('\n').count(), 1, "{line}");
        assert_eq!(requested(&line), Some(wanted));
        let nameless = Wanted::default();
        assert_eq!(requested(&request(&nameless)), Some(nameless));
        assert_eq!(
            requested("ASK new first - -"),
            Some(Wanted {
                step: Step::New,
                ..Wanted::default()
            })
        );
        for wrong in ["HELLO", "ASK maybe first - - +", "ASK", "ASK passphrase"] {
            assert_eq!(requested(wrong), None, "{wrong}");
        }
    }

    /// Says `lines` to a pinentry whose user gives what `given` says, in
    /// turn, and returns all it said back, and what it was asked to ask.
    fn spoken(lines: &[&str], given: Vec<Given>) -> (Vec<String>, Vec<Wanted>) {
        let mut told = Told::default();
        let mut given = given.into_iter();
        let mut asked = Vec::new();
        let mut ask = |wanted: &Wanted| {
            asked.push(wanted.clone());
            given.next().unwrap_or(Given::Cancelled)
        };
        let lookup = |grip: &str| listed_key(LISTED, grip);
        let mut said = Vec::new();
        for line in lines {
            let (replies, last) = reply(&mut told, line, &mut ask, &lookup);
            said.extend(replies);
            if last {
                break;
            }
        }
        (said, asked)
    }

    #[test]
    fn a_passphrase_is_asked_of_the_key_named_and_given_back() {
        let lines = [
            "OPTION ttyname=/dev/pts/1",
            "SETDESC Type your login password",
            "SETKEYINFO n/1111111111111111111111111111111111111111",
            "SETPROMPT Password:",
            "GETPIN",
            "BYE",
            "NOP",
        ];
        let (said, asked) = spoken(&lines, vec![Given::Typed("50% sure".to_owned())]);
        assert_eq!(said, ["OK", "OK", "OK", "OK", "D 50%25 sure", "OK", "OK"]);
        assert_eq!(
            asked,
            [Wanted {
                key: Some(alice()),
                step: Step::Passphrase,
                again: false,
                said: "Type your login password".to_owned(),
            }]
        );
    }

    #[test]
    fn after_a_wrong_one_the_next_is_asked_for_as_after_a_wrong_one_and_no_more() {
        let lines = [
            "SETERROR Bad passphrase",
            "GETPIN",
            "GETPIN",
            "SETERROR",
            "GETPIN",
        ];
        let given = vec![
            Given::Typed("a".to_owned()),
            Given::Typed("b".to_owned()),
            Given::Typed("c".to_owned()),
        ];
        let (_, asked) = spoken(&lines, given);
        let again: Vec<bool> = asked.iter().map(|wanted| wanted.again).collect();
        assert_eq!(again, [true, false, false]);
    }

    #[test]
    fn a_new_passphrase_is_asked_for_twice_and_given_only_if_the_same() {
        let same = vec![
            Given::Typed("new".to_owned()),
            Given::Typed("new".to_owned()),
        ];
        let (said, asked) = spoken(&["SETREPEAT", "GETPIN"], same);
        assert_eq!(said, ["OK", "D new", "S PIN_REPEATED", "OK"]);
        let steps: Vec<Step> = asked.iter().map(|wanted| wanted.step).collect();
        assert_eq!(steps, [Step::New, Step::NewAgain]);

        let different = vec![
            Given::Typed("new".to_owned()),
            Given::Typed("other".to_owned()),
        ];
        let (said, _) = spoken(&["SETREPEAT", "GETPIN"], different);
        assert_eq!(said, ["OK", CANCELLED]);
        let gave_up = vec![Given::Typed("new".to_owned()), Given::Cancelled];
        let (said, _) = spoken(&["SETREPEAT", "GETPIN", "GETPIN"], gave_up);
        assert_eq!(said[1], CANCELLED);
        // Once asked, the next is of the key again, not a new one.
        assert_eq!(said[2], CANCELLED);
    }

    #[test]
    fn given_up_on_or_with_nowhere_to_ask_gnupg_is_told_so() {
        let (said, _) = spoken(&["GETPIN"], vec![Given::Cancelled]);
        assert_eq!(said, [CANCELLED]);
        let (said, _) = spoken(&["GETPIN"], vec![Given::Nowhere]);
        assert_eq!(said, [NO_PROMPT]);
    }

    #[test]
    fn a_key_it_cannot_find_or_none_named_is_asked_of_as_unnamed() {
        let lines = [
            "SETKEYINFO s/9999",
            "GETPIN",
            "SETKEYINFO --clear",
            "GETPIN",
            "RESET",
            "GETPIN",
        ];
        let given = vec![Given::Cancelled, Given::Cancelled, Given::Cancelled];
        let (_, asked) = spoken(&lines, given);
        assert!(asked.iter().all(|wanted| wanted.key.is_none()), "{asked:?}");
    }

    #[test]
    fn a_program_asking_to_be_confirmed_is_told_no_and_one_saying_something_is_heard() {
        let lines = ["CONFIRM", "CONFIRM --one-button", "MESSAGE", "WHATEVER"];
        let (said, asked) = spoken(&lines, Vec::new());
        assert_eq!(said, [NOT_DONE, "OK", "OK", UNKNOWN]);
        assert!(asked.is_empty(), "nothing is put to the user");
    }

    /// Where what is written goes nowhere, as to a `gpg-agent` that has
    /// gone: it is taken, and then cannot be sent.
    struct Gone;

    impl Write for Gone {
        fn write(&mut self, written: &[u8]) -> std::io::Result<usize> {
            Ok(written.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn it_greets_answers_and_stops_once_told_or_once_nobody_hears() {
        let mut said = Vec::new();
        let asked = "GETINFO flavor\nGETPIN\nBYE\nGETINFO flavor\n".as_bytes();
        serve(asked, &mut said, (None, None));
        let said = String::from_utf8(said).unwrap();
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(
            lines,
            ["OK Pleased to meet you", "D omorfos", "OK", NO_PROMPT, "OK"]
        );
        // Said to nobody, it goes as soon as it is heard by nobody.
        serve("GETINFO flavor\n".as_bytes(), Gone, (None, None));
        let nowhere = Path::new("/nowhere/door");
        let nowhere = (Some(nowhere), None);
        assert_eq!(asked_at(nowhere, &Wanted::default()), Given::Nowhere);
    }

    #[test]
    fn it_says_what_it_is_when_asked() {
        let (said, _) = spoken(
            &["GETINFO flavor", "GETINFO pid", "GETINFO ttyinfo"],
            Vec::new(),
        );
        assert_eq!(said[0], "D omorfos");
        assert!(said[2].starts_with("D "), "{said:?}");
        assert_eq!(said[4], "OK", "nothing of what it does not say");
    }
}
