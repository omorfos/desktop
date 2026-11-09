//! What omorfos is made with that is someone else's, as `CREDITS.md`
//! lists it, for "About" to show: each table of the file, under its
//! heading, a row a credit.

/// One thing of someone else's: what it is, whose, and under what licence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    /// What it is, as "Inter".
    pub what: String,
    /// Whose it is.
    pub by: String,
    /// The licence it is under.
    pub licence: String,
}

/// The credits under one heading of the file, as "Built in".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credits {
    /// The heading.
    pub heading: String,
    /// The rows of its table, in their order.
    pub credits: Vec<Credit>,
}

/// The tables of `markdown`, each with the heading it is under, and each
/// row's first three columns as words alone, without links or code marks.
/// A heading with no table is left out. A row with fewer columns has the
/// rest empty, and so is seen to be wrong rather than missing.
#[must_use]
pub fn credits(markdown: &str) -> Vec<Credits> {
    let mut all: Vec<Credits> = Vec::new();
    // Whether the line before was of a table, so that this one is not its
    // first, which says what the columns are.
    let mut in_table = false;
    for line in markdown.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            all.push(Credits {
                heading: heading.trim().to_owned(),
                credits: Vec::new(),
            });
        }
        let row = line.trim().strip_prefix('|');
        let header = row.is_some() && !in_table;
        in_table = row.is_some();
        let cells: Vec<String> = row
            .filter(|_| !header)
            .into_iter()
            .flat_map(|row| row.split('|'))
            .map(|cell| plain(cell.trim()))
            .collect();
        let rule = cells.iter().all(|cell| cell.chars().all(|c| c == '-'));
        if rule {
            continue;
        }
        let mut cells = cells.into_iter();
        let mut cell = || cells.next().unwrap_or_default();
        let credit = Credit {
            what: cell(),
            by: cell(),
            licence: cell(),
        };
        // A table is under a heading; one before the first is not a
        // credit's, and the test of the file would find it missing.
        let section = all.last_mut().into_iter();
        section.for_each(|section| section.credits.push(credit.clone()));
    }
    all.retain(|section| !section.credits.is_empty());
    all
}

/// `text` as it reads: each link as its words, and code as plain words.
fn plain(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let link = rest[start..].find("](").and_then(|middle| {
            rest[start + middle..]
                .find(')')
                .map(|end| (middle, middle + end))
        });
        let Some((middle, end)) = link else {
            break;
        };
        plain.push_str(&rest[..start]);
        plain.push_str(&rest[start + 1..start + middle]);
        rest = &rest[start + end + 1..];
    }
    plain.push_str(rest);
    plain.replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Credits

Words before any table.

## Built in

| What | By | Licence | Used for |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | Rasmus Andersson | SIL Open Font Licence 1.1 | The font |
| `zone1970.tab` | The [tz](https://tz) contributors | Public domain | Zones |

Words between.

## Thanks

No table here.

## Short

| What | By |
|---|---|
| Only | Two |
";

    #[test]
    fn each_table_is_its_heading_and_its_rows_in_words_alone() {
        let all = credits(SAMPLE);
        let headings: Vec<&str> = all.iter().map(|section| section.heading.as_str()).collect();
        assert_eq!(
            headings,
            ["Built in", "Short"],
            "a heading with no table is left out"
        );
        assert_eq!(
            all[0].credits,
            [
                Credit {
                    what: "Inter".to_owned(),
                    by: "Rasmus Andersson".to_owned(),
                    licence: "SIL Open Font Licence 1.1".to_owned(),
                },
                Credit {
                    what: "zone1970.tab".to_owned(),
                    by: "The tz contributors".to_owned(),
                    licence: "Public domain".to_owned(),
                },
            ]
        );
        let short = &all[1].credits[0];
        assert_eq!(
            (short.what.as_str(), short.licence.as_str()),
            ("Only", ""),
            "seen to be wrong"
        );
    }

    #[test]
    fn a_link_that_is_never_closed_is_kept_as_it_is_written() {
        assert_eq!(
            plain("see [this](there and more"),
            "see [this](there and more"
        );
        assert_eq!(plain("[a](b) and [c"), "a and [c");
    }

    #[test]
    fn every_row_of_the_credits_is_read_whole() {
        let file = include_str!("../../../CREDITS.md");
        let rows = file.lines().filter(|line| line.starts_with("| ")).count();
        let tables = file.lines().filter(|line| line.starts_with("|---")).count();
        let all = credits(file);
        let read: Vec<&Credit> = all.iter().flat_map(|section| &section.credits).collect();
        assert_eq!(
            read.len(),
            rows - tables,
            "every row but each table's first"
        );
        for credit in &read {
            let whole = [&credit.what, &credit.by, &credit.licence];
            assert!(whole.iter().all(|cell| !cell.is_empty()), "{credit:?}");
            assert!(whole.iter().all(|cell| !cell.contains("](")), "{credit:?}");
        }
        assert!(read.iter().any(|credit| credit.what == "Inter"));
        assert!(all.iter().any(|section| section.heading == "Built in"));
    }
}
