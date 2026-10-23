//! What a helper that asks PAM says: polkit's own, and the one the lock
//! screen starts for each way of unlocking. Each line is a word first and
//! then what PAM asked or said, until it says whether that was right.

/// What the helper says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A question whose answer is not to be shown as it is typed, as a
    /// password is not.
    Secret(String),
    /// A question whose answer may be shown.
    Open(String),
    /// Something gone wrong to tell the user.
    Error(String),
    /// Something to tell the user.
    Info(String),
    /// That was right: the program may.
    Right,
    /// That was not, or it could not be asked.
    Wrong,
    /// What was to be asked cannot be here: the module that asks it is
    /// not there, or the user has nothing for it to ask, as no finger
    /// enrolled.
    Unavailable,
}

/// What one line from the helper says, if it says anything this knows.
#[must_use]
pub fn heard(line: &str) -> Option<Said> {
    let line = line.trim_end_matches(['\n', '\r']);
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.to_owned();
    match word {
        "PAM_PROMPT_ECHO_OFF" => Some(Said::Secret(rest)),
        "PAM_PROMPT_ECHO_ON" => Some(Said::Open(rest)),
        "PAM_ERROR_MSG" => Some(Said::Error(rest)),
        "PAM_TEXT_INFO" => Some(Said::Info(rest)),
        "SUCCESS" => Some(Said::Right),
        "FAILURE" => Some(Said::Wrong),
        "UNAVAILABLE" => Some(Said::Unavailable),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_says_what_pam_asks_or_says_or_how_it_ended() {
        let said = |line| heard(line);
        assert_eq!(
            said("PAM_PROMPT_ECHO_OFF Password: \n"),
            Some(Said::Secret("Password: ".to_owned()))
        );
        assert_eq!(
            said("PAM_PROMPT_ECHO_ON Login:"),
            Some(Said::Open("Login:".to_owned()))
        );
        assert_eq!(
            said("PAM_ERROR_MSG Account locked"),
            Some(Said::Error("Account locked".to_owned()))
        );
        assert_eq!(
            said("PAM_TEXT_INFO Touch the key"),
            Some(Said::Info("Touch the key".to_owned()))
        );
        assert_eq!(said("SUCCESS\n"), Some(Said::Right));
        assert_eq!(said("FAILURE"), Some(Said::Wrong));
        assert_eq!(said("UNAVAILABLE"), Some(Said::Unavailable));
        assert_eq!(said("something else"), None);
        assert_eq!(said(""), None);
    }
}
