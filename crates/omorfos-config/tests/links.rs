//! Keeps the links between the repository's documents leading somewhere:
//! each relative link in a Markdown file to a file or directory that is
//! there, and to a heading in it where it names one, as GitHub makes them.
//! Links to elsewhere, as `https:`, are passed over: the gates run without
//! the network.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// The links in the Markdown `text` that may lead to a file, outside code:
/// those written inline, `[text](target)`, and those defined for
/// reference, `[name]: target`. One with a scheme, as `https:`, is not.
fn links(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        // What is between backticks is code, and every other piece is not.
        let prose = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        let inline = prose
            .split("](")
            .skip(1)
            .map(|after| after.split(')').next().unwrap_or_default());
        let defined = prose
            .trim_start()
            .strip_prefix('[')
            .and_then(|rest| rest.split_once("]: "));
        let targets = inline.chain(defined.map(|(_, target)| target));
        // A title may follow the target, after a space.
        let targets = targets.filter_map(|target| target.split_whitespace().next());
        found.extend(targets.filter(|target| !schemed(target)).map(str::to_owned));
    }
    found
}

/// Whether `target` names where it leads by a scheme, as `https:` or
/// `mailto:` do.
fn schemed(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|first: char| first.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    })
}

/// The anchors GitHub gives the headings of the Markdown `text`: each
/// lowercased, with what is neither a letter, a digit, `-` nor `_` taken
/// out and each space made `-`; a second of the same with `-1` after it,
/// and so on.
fn anchors(text: &str) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut fenced = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let heading = line.trim_start_matches('#');
        if fenced || heading.len() == line.len() || !heading.starts_with(' ') {
            continue;
        }
        let anchor: String = heading
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || " -_".contains(*c))
            .flat_map(char::to_lowercase)
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        let times = seen.entry(anchor.clone()).or_default();
        found.push(if *times == 0 {
            anchor
        } else {
            format!("{anchor}-{times}")
        });
        *times += 1;
    }
    found
}

/// Whether `link`, written in the document at `from`, leads somewhere: to
/// a file or directory that is there, and, where it names a heading, to a
/// document that has it.
fn leads(from: &Path, link: &str) -> bool {
    let (path, heading) = link.split_once('#').unwrap_or((link, ""));
    let target = if path.is_empty() {
        from.to_path_buf()
    } else {
        from.parent().unwrap_or(from).join(path)
    };
    let headed = || {
        let text = fs::read_to_string(&target).unwrap_or_default();
        anchors(&text).iter().any(|anchor| anchor == heading)
    };
    target.exists() && (heading.is_empty() || headed())
}

/// Every Markdown document under `directory`, but in what is built and in
/// git's own.
fn documents(directory: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(directory).into_iter().flatten().flatten();
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() && name != "target" && name != ".git" {
            found.extend(documents(&path));
        } else if path.extension().is_some_and(|extension| extension == "md") {
            found.push(path);
        }
    }
    found
}

/// Each link in the documents under `directory` that leads nowhere, with
/// the document it is in, as a path from `directory`.
fn broken(directory: &Path) -> Vec<(PathBuf, String)> {
    let mut broken = Vec::new();
    for document in documents(directory) {
        let text = fs::read_to_string(&document).unwrap_or_default();
        let named = document
            .strip_prefix(directory)
            .unwrap_or(&document)
            .to_path_buf();
        let nowhere = links(&text)
            .into_iter()
            .filter(|link| !leads(&document, link));
        broken.extend(nowhere.map(|link| (named.clone(), link)));
    }
    broken.sort();
    broken
}

#[test]
fn every_link_between_the_documents_leads_somewhere() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let found = documents(&repository).len();
    assert!(found > 50, "only {found} documents found");
    assert_eq!(broken(&repository), []);
}

#[test]
fn a_link_that_leads_nowhere_is_found_and_one_that_leads_somewhere_is_not() {
    let place = TempDir::new().unwrap();
    let write = |path: &str, text: &str| {
        let path = place.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(
        "README.md",
        "# Here\n\
         [there](docs/b.md), [up](docs/b.md#a-heading-it-has), [again](docs/b.md#twice-1)\n\
         [a directory](docs/) and [here](#here) and [elsewhere](https://example.org/x.md)\n\
         [gone](docs/c.md \"a title\"), [no heading](docs/b.md#none) and [nor here](#nowhere)\n\
         `[in code](docs/d.md)`\n\
         ```\n[fenced](docs/e.md)\n## Not a heading\n```\n\
         [named]: docs/f.md\n\
         #hashtag\n",
    );
    write("docs/b.md", "## A heading, it has!\n## Twice\n## Twice\n");
    write("target/built.md", "[nowhere](nowhere.md)\n");
    let named = |link: &str| (PathBuf::from("README.md"), link.to_owned());
    assert_eq!(
        broken(place.path()),
        [
            named("#nowhere"),
            named("docs/b.md#none"),
            named("docs/c.md"),
            named("docs/f.md"),
        ]
    );
}
