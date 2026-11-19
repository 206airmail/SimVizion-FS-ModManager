//! Dev tool: prints what discovery finds on this machine. Read-only.
//!
//! Run with `cargo run -p fsmm-core --example discover`. It never prints the contents of
//! `gameSettings.xml` (it holds plain-text multiplayer passwords), only whether the mods
//! override is active.

#[cfg(windows)]
fn main() {
    use fsmm_core::discovery::{
        current_desc_version, effective_mods_dir, find_installs, read_log_summary,
        read_mods_override, running, suggest_profiles, user_dir, WindowsProbe,
    };

    let probe = WindowsProbe;

    let installs = find_installs(&probe);
    println!("installs: {}", installs.len());
    for install in &installs {
        println!(
            "  {:?}  {}  version {}",
            install.store,
            install.dir.display(),
            install.version.as_deref().unwrap_or("(no VERSION file)")
        );
    }
    let installed_version = installs.first().and_then(|i| i.version.as_deref());

    let user = user_dir(&probe);
    match &user {
        Some(dir) => println!("user folder: {}", dir.display()),
        None => println!("user folder: not found"),
    }

    let mods_override = user
        .as_ref()
        .map(|u| read_mods_override(&u.join("gameSettings.xml")));
    match &mods_override {
        Some(Ok(o)) => println!("mods override active: {}", o.active),
        Some(Err(e)) => println!("gameSettings.xml: {e}"),
        None => println!("gameSettings.xml: no user folder"),
    }
    let effective = effective_mods_dir(
        user.as_deref(),
        mods_override.as_ref().and_then(|r| r.as_ref().ok()),
    );
    match &effective {
        Some(dir) => println!(
            "effective mods folder: {} (exists: {})",
            dir.display(),
            dir.is_dir()
        ),
        None => println!("effective mods folder: unknown"),
    }

    match user.as_ref().map(|u| read_log_summary(&u.join("log.txt"))) {
        Some(Ok(log)) => {
            println!(
                "log: version {}, language {}, ModDesc Version {}",
                log.game_version.as_deref().unwrap_or("?"),
                log.language.as_deref().unwrap_or("?"),
                log.mod_desc_version
                    .map_or_else(|| "?".to_owned(), |v| v.to_string())
            );
            let current = current_desc_version(&log, installed_version);
            println!(
                "log is current for the installed version: {}",
                if current.is_some() { "yes" } else { "no" }
            );
        }
        Some(Err(e)) => println!("log: {e}"),
        None => println!("log: no user folder"),
    }

    let run = running(&probe);
    println!("game running: {}", run.game);
    println!("dedicated server running: {}", run.server);

    println!("suggested profiles:");
    if let Some(user) = &user {
        for profile in suggest_profiles(user) {
            println!("  {}", profile.display());
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("discover only runs on Windows");
}
