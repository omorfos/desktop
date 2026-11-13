//! Manual pages, made from what each program already says of itself, so
//! that `man` and `--help` cannot disagree: written in roff, as man(7) has
//! it, and kept in `man/` by a test that makes them again.
//!
//! A page names no version and no date, which would change with every
//! release and leave the pages kept behind; `man` shows neither as missing.

use std::path::Path;
use std::{fs, io};

use crate::help::{ASKED, About};

/// The pages every program's page points to, at its end.
const SEE_ALSO: &[&str] = &[
    "omorfos(1)",
    "omorfosctl(1)",
    "omorfos-config(5)",
    "omorfos-keys(5)",
    "omorfos-rules(5)",
    "omorfos-menu(5)",
    "omorfos-theme(5)",
];

/// A manual page as it is written, a part at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    roff: String,
}

impl Page {
    /// A page for `name` in `section` of the manual, which is `what`, said
    /// in a line.
    #[must_use]
    pub fn new(name: &str, section: u8, what: &str) -> Self {
        let title = escaped(&name.to_uppercase());
        let what = what.trim_end_matches('.');
        let mut page = Self {
            // Neither hyphenated nor stretched to both edges: names and
            // options are to be read whole, as they are typed. groff's
            // macros set the adjustment again from `AD` at some parts, so
            // that is set as well.
            roff: format!(
                ".TH {title} {section} \"\" omorfos \"omorfos manual\"\n.nh\n.ad l\n.ds AD l\n"
            ),
        };
        page.section("Name");
        page.line(&format!("{} \\- {}", escaped(name), escaped(what)));
        page
    }

    /// Begins a part titled `title`.
    pub fn section(&mut self, title: &str) {
        self.roff.push_str(".SH ");
        self.roff.push_str(&escaped(&title.to_uppercase()));
        self.roff.push('\n');
    }

    /// A paragraph of `text`, in which what is between backticks is bold,
    /// as it is to be typed.
    pub fn paragraph(&mut self, text: &str) {
        self.roff.push_str(".PP\n");
        self.line(&marked(text));
    }

    /// `term`, in bold, and what it is, `text`, set in under it.
    pub fn entry(&mut self, term: &str, text: &str) {
        self.roff.push_str(".TP\n");
        self.line(&format!("\\fB{}\\fR", escaped(term)));
        self.line(&marked(text));
    }

    /// The values the entry before may be given, each with what it means,
    /// set in under it; one that means nothing more than itself alone.
    pub fn values(&mut self, values: &[(String, String)]) {
        self.roff.push_str(".RS\n");
        for (value, means) in values {
            self.roff.push_str(".TP\n");
            self.line(&format!("\\fB{}\\fR", escaped(value)));
            if !means.is_empty() {
                self.line(&marked(means));
            }
        }
        self.roff.push_str(".RE\n");
    }

    /// `text` as it is, a line for each of its lines, not filled, as code
    /// and files are shown.
    pub fn verbatim(&mut self, text: &str) {
        self.roff.push_str(".PP\n.nf\n.RS 4\n");
        for line in text.lines() {
            self.line(&escaped(line));
        }
        self.roff.push_str(".RE\n.fi\n");
    }

    /// The pages it is to be read with, by name and section, as
    /// `omorfos(1)`.
    pub fn see_also(&mut self, pages: &[&str]) {
        self.section("See also");
        self.roff.push_str(".PP\n");
        let named: Vec<String> = pages
            .iter()
            .map(|page| match page.split_once('(') {
                Some((name, section)) => format!("\\fB{}\\fR({section}", escaped(name)),
                None => escaped(page),
            })
            .collect();
        self.line(&named.join(", "));
    }

    /// The page, written.
    #[must_use]
    pub fn finish(self) -> String {
        self.roff
    }

    /// Writes `text`, already escaped and marked, as a line of its own; one
    /// that would begin with a full stop or an apostrophe, which roff
    /// would take for a request, is begun with a mark that is not drawn.
    fn line(&mut self, text: &str) {
        if text.starts_with(['.', '\'']) {
            self.roff.push_str("\\&");
        }
        self.roff.push_str(text);
        self.roff.push('\n');
    }
}

/// `text` with what roff gives a meaning to escaped: the backslash, and
/// the hyphen, which is drawn as a minus that can be typed, as options
/// and names are, and not as a typographic hyphen.
#[must_use]
pub fn escaped(text: &str) -> String {
    text.replace('\\', "\\e").replace('-', "\\-")
}

/// `text` escaped, with what is between backticks in bold.
fn marked(text: &str) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut marked = String::new();
    for (index, piece) in words.split('`').enumerate() {
        if index % 2 == 1 {
            marked.push_str("\\fB");
            marked.push_str(&escaped(piece));
            marked.push_str("\\fR");
        } else {
            marked.push_str(&escaped(piece));
        }
    }
    marked
}

/// Whether the page kept at `path` is `made`, as the tests that keep the
/// pages ask; with `bless`, as `OMORFOS_BLESS` asks of them, it is written
/// so first.
///
/// # Errors
///
/// Fails if it is to be written and cannot be.
pub fn kept(path: &Path, made: &str, bless: bool) -> io::Result<bool> {
    if bless {
        fs::write(path, made)?;
    }
    Ok(fs::read_to_string(path).is_ok_and(|kept| kept == made))
}

/// The manual page of the program `about` tells of, in section 1: what it
/// is and how it is used, more of it, its commands and options, examples,
/// and the pages to read with it.
#[must_use]
pub fn page(about: &About) -> String {
    let mut page = Page::new(about.name, 1, about.what);
    page.section("Synopsis");
    page.paragraph(&format!("`{}{}`", about.name, about.usage));
    // What it is, the page's name has said already.
    if !about.notes.is_empty() {
        page.section("Description");
        for paragraph in about.notes.split("\n\n") {
            page.paragraph(paragraph);
        }
    }
    if !about.commands.is_empty() {
        page.section("Commands");
        for (said, does) in about.commands {
            page.entry(said, does);
        }
    }
    page.section("Options");
    for (said, does) in about.options.iter().chain(ASKED) {
        page.entry(said, does);
    }
    if !about.examples.is_empty() {
        page.section("Examples");
        page.verbatim(&about.examples.join("\n"));
    }
    let own = format!("{}(1)", about.name);
    let others: Vec<&str> = SEE_ALSO
        .iter()
        .copied()
        .filter(|other| *other != own)
        .collect();
    page.see_also(&others);
    page.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_roff_gives_a_meaning_to_is_escaped_and_a_line_never_begins_a_request() {
        assert_eq!(escaped("a-b \\n"), "a\\-b \\en");
        let mut page = Page::new("omorfos-tool", 1, "A tool.");
        page.paragraph(".hidden and 'quoted', with `--to` in it");
        page.values(&[
            ("left".to_owned(), "To the `left`".to_owned()),
            ("none".to_owned(), String::new()),
        ]);
        page.verbatim(".not a request\n'nor this\nplain");
        let roff = page.finish();
        assert!(roff.starts_with(".TH OMORFOS\\-TOOL 1 \"\" omorfos \"omorfos manual\"\n.nh\n.ad l\n.ds AD l\n.SH NAME\nomorfos\\-tool \\- A tool\n"), "{roff}");
        assert!(
            roff.contains("\n\\&.hidden and 'quoted', with \\fB\\-\\-to\\fR in it\n"),
            "{roff}"
        );
        assert!(
            roff.contains("\n\\&.not a request\n\\&'nor this\nplain\n"),
            "{roff}"
        );
        assert!(
            !roff.lines().any(|line| line.starts_with(".hidden")),
            "{roff}"
        );
        assert!(
            roff.contains(
                "\n.RS\n.TP\n\\fBleft\\fR\nTo the \\fBleft\\fR\n.TP\n\\fBnone\\fR\n.RE\n"
            ),
            "{roff}"
        );
    }

    #[test]
    fn a_page_is_kept_only_as_it_is_made_and_written_so_when_asked() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("x.1");
        assert!(!kept(&path, "page", false).unwrap(), "none there");
        assert!(kept(&path, "page", true).unwrap());
        assert!(kept(&path, "page", false).unwrap());
        assert!(!kept(&path, "other", false).unwrap(), "out of date");
        let nowhere = directory.path().join("missing/x.1");
        assert!(kept(&nowhere, "page", true).is_err());
    }

    #[test]
    fn a_programs_page_says_what_its_help_does_and_points_to_the_others() {
        let tool = About {
            name: "omorfosctl",
            what: "A tool.",
            usage: " [COMMAND]",
            commands: &[("louder", "Turn it `up`")],
            options: &[("--to WHERE", "Where it goes")],
            examples: &["omorfosctl louder"],
            notes: "A first paragraph,\nin two lines.\n\nA second.",
            version: "1.2.3",
        };
        let roff = page(&tool);
        let expected = "\
.TH OMORFOSCTL 1 \"\" omorfos \"omorfos manual\"
.nh
.ad l
.ds AD l
.SH NAME
omorfosctl \\- A tool
.SH SYNOPSIS
.PP
\\fBomorfosctl [COMMAND]\\fR
.SH DESCRIPTION
.PP
A first paragraph, in two lines.
.PP
A second.
.SH COMMANDS
.TP
\\fBlouder\\fR
Turn it \\fBup\\fR
.SH OPTIONS
.TP
\\fB\\-\\-to WHERE\\fR
Where it goes
.TP
\\fB\\-h, \\-\\-help\\fR
Print this help
.TP
\\fB\\-V, \\-\\-version\\fR
Print the version
.SH EXAMPLES
.PP
.nf
.RS 4
omorfosctl louder
.RE
.fi
.SH SEE ALSO
.PP
\\fBomorfos\\fR(1), \\fBomorfos\\-config\\fR(5), \\fBomorfos\\-keys\\fR(5), \\fBomorfos\\-rules\\fR(5), \\fBomorfos\\-menu\\fR(5), \\fBomorfos\\-theme\\fR(5)
";
        assert_eq!(roff, expected);
        // A program with neither commands nor examples has no part for
        // them, and points to every other page.
        let plain = About {
            name: "omorfos-panel",
            commands: &[],
            examples: &[],
            notes: "",
            ..tool
        };
        let roff = page(&plain);
        assert!(
            !roff.contains(".SH COMMANDS")
                && !roff.contains(".SH EXAMPLES")
                && !roff.contains(".SH DESCRIPTION"),
            "{roff}"
        );
        assert!(
            roff.contains(
                "\n\\fBomorfos\\fR(1), \\fBomorfosctl\\fR(1), \\fBomorfos\\-config\\fR(5),"
            ),
            "{roff}"
        );
        let mut unnamed = Page::new("x", 5, "X");
        unnamed.see_also(&["somewhere"]);
        assert!(unnamed.finish().ends_with("SEE ALSO\n.PP\nsomewhere\n"));
    }
}
