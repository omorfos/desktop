//! What the launcher finds things in (ADR 109): each a source answering
//! what is typed with what it found, best first, for a section of its own.
//! The programs installed and sums are the first two; files, actions and
//! the rest come as sources of their own.

use omorfos_config::Command;

use crate::{App, Numbers};

/// What the launcher shows something under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    /// A sum's answer.
    Sum,
    /// The programs installed.
    Programs,
}

/// Something found: what it is called, what it says of itself, its icon,
/// and what picking it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// Its name, as shown.
    pub name: String,
    /// What it says of itself, in a few words; empty if nothing.
    pub says: String,
    /// Its icon, by name or path, if it has one.
    pub icon: Option<String>,
    /// What Enter or a click on it asks of the compositor.
    pub does: Does,
}

/// What picking something asks of the compositor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Does {
    /// Start this program with these arguments.
    Spawn(Vec<String>),
    /// Put this on the clipboard.
    Copy(String),
}

/// What finds things for the launcher.
pub(crate) trait Source {
    /// What what it finds is shown under.
    fn section(&self) -> Section;
    /// What it finds for `query`, best first.
    fn find(&self, query: &str) -> Vec<Found>;
}

/// The programs installed, by their desktop entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Programs {
    apps: Vec<App>,
    /// Whether what programs say of themselves is looked through too.
    descriptions: bool,
}

impl Programs {
    /// The programs `apps`, in the order they are given, looking through
    /// what they say of themselves too if `descriptions`.
    pub(crate) const fn new(apps: Vec<App>, descriptions: bool) -> Self {
        Self { apps, descriptions }
    }
}

impl Source for Programs {
    fn section(&self) -> Section {
        Section::Programs
    }

    /// Those whose name begins with what is typed first, then those that
    /// have it anywhere in their name, then, if what programs say of
    /// themselves is looked through, those that have it there; each lot
    /// in the order the programs came in. Nothing typed, every program.
    fn find(&self, query: &str) -> Vec<Found> {
        let query = query.to_lowercase();
        let rank = |app: &App| {
            let name = app.name.to_lowercase();
            let says = self.descriptions && app.description.to_lowercase().contains(&query);
            let ranks = [name.starts_with(&query), name.contains(&query), says];
            ranks.into_iter().position(|matches| matches)
        };
        let ranked = self
            .apps
            .iter()
            .filter_map(|app| rank(app).map(|rank| (rank, app)));
        let mut ranked: Vec<(usize, &App)> = ranked.collect();
        // The sort is stable, so each lot stays in the order it came in.
        ranked.sort_by_key(|(rank, _)| *rank);
        ranked.into_iter().map(|(_, app)| found(app)).collect()
    }
}

/// A program as something found: one that needs a terminal is started in
/// the one the built-in bindings start.
fn found(app: &App) -> Found {
    let terminal = app.terminal.then(Command::terminal);
    let terminal = terminal.iter().map(|command| command.program().to_owned());
    let command = terminal.chain(app.command.iter().cloned()).collect();
    Found {
        name: app.name.clone(),
        says: app.description.clone(),
        icon: Some(app.icon.clone()).filter(|icon| !icon.is_empty()),
        does: Does::Spawn(command),
    }
}

/// Sums typed, worked out as numbers are written where the user is
/// (ADR 86).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sums {
    numbers: Numbers,
}

impl Sums {
    /// Sums read and answered as `numbers` has numbers written.
    pub(crate) const fn new(numbers: Numbers) -> Self {
        Self { numbers }
    }
}

impl Source for Sums {
    fn section(&self) -> Section {
        Section::Sum
    }

    /// The answer, if what is typed is a sum: shown after `=`, copied as
    /// it is.
    fn find(&self, query: &str) -> Vec<Found> {
        let answer = omorfos_sums::answer(query, self.numbers);
        let found = answer.map(|answer| Found {
            name: format!("= {answer}"),
            says: String::new(),
            icon: None,
            does: Does::Copy(answer),
        });
        found.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, terminal: bool) -> App {
        App {
            name: name.to_owned(),
            command: vec![name.to_lowercase()],
            terminal,
            description: format!("A {} of sorts", name.to_lowercase()),
            icon: if terminal {
                String::new()
            } else {
                name.to_lowercase()
            },
        }
    }

    fn names(found: &[Found]) -> Vec<&str> {
        found.iter().map(|found| found.name.as_str()).collect()
    }

    #[test]
    fn sources_rank_as_the_launcher_always_has() {
        let apps = ["Profile", "Calculator", "Files", "Firefox"].map(|name| app(name, false));
        let programs = Programs::new(apps.to_vec(), false);
        assert_eq!(programs.section(), Section::Programs);
        assert_eq!(names(&programs.find("fi")), ["Files", "Firefox", "Profile"]);
        assert_eq!(programs.find("").len(), 4, "nothing typed, every one");
        assert_eq!(names(&programs.find("sorts")), [] as [&str; 0]);
        let described = Programs::new(apps.to_vec(), true);
        assert_eq!(names(&described.find("calculator of")), ["Calculator"]);
        let files = &programs.find("files")[0];
        assert_eq!(files.icon.as_deref(), Some("files"));
        assert_eq!(files.does, Does::Spawn(vec!["files".to_owned()]));
    }

    #[test]
    fn a_program_that_needs_a_terminal_is_started_in_one_and_has_no_icon_if_none() {
        let programs = Programs::new(vec![app("Top", true)], false);
        let top = &programs.find("top")[0];
        assert_eq!(top.icon, None);
        let terminal = Command::terminal().program().to_owned();
        assert_eq!(top.does, Does::Spawn(vec![terminal, "top".to_owned()]));
    }

    #[test]
    fn a_sum_is_answered_to_be_copied_and_anything_else_is_not() {
        let sums = Sums::new(Numbers::Point);
        assert_eq!(sums.section(), Section::Sum);
        let found = sums.find("6*7");
        assert_eq!(names(&found), ["= 42"]);
        assert_eq!(found[0].does, Does::Copy("42".to_owned()));
        assert_eq!(sums.find("pi"), []);
    }
}
