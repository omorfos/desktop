//! The `omorfos-lock` executable itself.

#[test]
fn asked_what_it_is_omorfos_lock_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-lock"),
        ("-h", "Usage: omorfos-lock"),
        ("--version", "omorfos-lock "),
        ("-V", "omorfos-lock "),
    ] {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_omorfos-lock"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
