//! Who is asked for their password: one of those polkit says may say yes,
//! the user at the keyboard if they are one, and otherwise the first.
//!
//! polkit names them as users and as groups, the administrators' group on
//! most systems. A group's members are read from the system's own lists
//! of users and groups, `/etc/passwd` and `/etc/group`, as text.

/// Who polkit says may say yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    /// A user, by number.
    User(u32),
    /// Every member of a group, by number.
    Group(u32),
}

/// A user, by name and number, as the system lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// What they log in as.
    pub name: String,
    /// Their number.
    pub uid: u32,
    /// The number of their own group.
    pub gid: u32,
}

/// The users `/etc/passwd` lists, as text: each line a name, a password
/// field, a number, a group and more, apart by `:`.
#[must_use]
pub fn users(passwd: &str) -> Vec<Person> {
    let person = |line: &str| {
        let mut fields = line.split(':');
        let name = fields.next().filter(|name| !name.is_empty())?;
        let uid = fields.nth(1)?.parse().ok()?;
        let gid = fields.next()?.parse().ok()?;
        Some(Person {
            name: name.to_owned(),
            uid,
            gid,
        })
    };
    passwd.lines().filter_map(person).collect()
}

/// The names `/etc/group` gives as members of the group numbered `gid`.
fn members(group: &str, gid: u32) -> Vec<&str> {
    let listed = group.lines().find_map(|line| {
        let mut fields = line.split(':');
        let number = fields.nth(2)?.parse::<u32>().ok()?;
        let members = fields.next()?;
        (number == gid).then_some(members)
    });
    let names = listed.unwrap_or_default().split(',').map(str::trim);
    names.filter(|name| !name.is_empty()).collect()
}

/// Everyone `identities` names, in the order named: a group's members, and
/// those whose own group it is.
#[must_use]
pub fn named(identities: &[Identity], users: &[Person], group: &str) -> Vec<Person> {
    let mut people: Vec<Person> = Vec::new();
    for identity in identities {
        let found: Vec<&Person> = match identity {
            Identity::User(uid) => users.iter().filter(|person| person.uid == *uid).collect(),
            Identity::Group(gid) => {
                let listed = members(group, *gid);
                let member =
                    |person: &&Person| person.gid == *gid || listed.contains(&person.name.as_str());
                users.iter().filter(member).collect()
            }
        };
        for person in found {
            if !people.contains(person) {
                people.push(person.clone());
            }
        }
    }
    people
}

/// Who to ask: the user numbered `me` if they are among `people`, and the
/// first of them otherwise.
#[must_use]
pub fn chosen(people: &[Person], me: u32) -> Option<&Person> {
    let mine = people.iter().find(|person| person.uid == me);
    mine.or_else(|| people.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD: &str = "root:x:0:0:root:/root:/bin/sh\n\
        rafal:x:1000:1000:Rafal,,,:/home/rafal:/usr/bin/fish\n\
        guest:x:1001:1001::/home/guest:/bin/sh\n\
        broken line\n\
        :x:5:5::/:/bin/sh\n\
        odd:x:many:1::/:/bin/sh\n\
        groupless:x:7\n\
        oddgroup:x:8:many::/:/bin/sh\n";

    const GROUP: &str =
        "root:x:0:\nshort:x\nsudo:x:27:rafal, guest\nwheel:x:10:\nbad:x:no:rafal\nnone:x:30\n";

    fn person(name: &str, uid: u32) -> Person {
        Person {
            name: name.to_owned(),
            uid,
            gid: uid,
        }
    }

    #[test]
    fn the_users_are_read_from_the_list_and_what_is_not_one_passed_over() {
        let read = users(PASSWD);
        assert_eq!(
            read,
            [
                person("root", 0),
                person("rafal", 1000),
                person("guest", 1001)
            ]
        );
    }

    #[test]
    fn a_group_names_its_members_and_those_whose_own_group_it_is() {
        let all = users(PASSWD);
        let admins = named(&[Identity::Group(27)], &all, GROUP);
        assert_eq!(admins, [person("rafal", 1000), person("guest", 1001)]);
        let own = named(&[Identity::Group(0)], &all, GROUP);
        assert_eq!(own, [person("root", 0)], "root's own group");
        assert_eq!(named(&[Identity::Group(10)], &all, GROUP), [], "nobody");
        assert_eq!(
            named(&[Identity::Group(99)], &all, GROUP),
            [],
            "no such group"
        );
    }

    #[test]
    fn everyone_is_named_once_in_the_order_given() {
        let all = users(PASSWD);
        let identities = [
            Identity::User(1001),
            Identity::Group(27),
            Identity::User(0),
            Identity::User(4242),
        ];
        let people = named(&identities, &all, GROUP);
        assert_eq!(
            people,
            [
                person("guest", 1001),
                person("rafal", 1000),
                person("root", 0)
            ]
        );
    }

    #[test]
    fn the_user_at_the_keyboard_is_asked_if_they_may_and_otherwise_the_first() {
        let people = [person("root", 0), person("rafal", 1000)];
        assert_eq!(chosen(&people, 1000), Some(&people[1]));
        assert_eq!(chosen(&people, 1001), Some(&people[0]));
        assert_eq!(chosen(&[], 1000), None);
    }
}
