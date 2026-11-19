//! Ops tests: temp folders only, never a real game folder. Fixtures are generated here and hold
//! a fake password so the tests can prove no error text leaks file contents.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

pub(crate) const SECRET: &str = "FAKE-SECRET-123";
const BOM_BYTES: [u8; 3] = [0xEF, 0xBB, 0xBF];
pub(crate) const TESTING_DIR: &str = r"D:\Users\Someone\Documents\My Games\FS\$testingMods";

pub(crate) fn override_line(active: &str, dir: &str) -> String {
    format!("    <modsDirectoryOverride active=\"{active}\" directory=\"{dir}\"/>")
}

/// A file shaped like the real one: XML declaration, root, the override on line 3, other
/// settings including passwords.
pub(crate) fn settings_text(override_line: &str, eol: &str) -> String {
    let lines = [
        r#"<?xml version="1.0" encoding="utf-8" standalone="no"?>"#,
        "<gameSettings>",
        override_line,
        "    <onlinePresenceName>Someone</onlinePresenceName>",
        &format!("    <onlinePassword>{SECRET}</onlinePassword>"),
        "    <multiplayer>",
        &format!("        <password>{SECRET}</password>"),
        "    </multiplayer>",
        "    <autoSave>true</autoSave>",
        "</gameSettings>",
    ];
    let mut text = lines.join(eol);
    text.push_str(eol);
    text
}

pub(crate) fn with_bom(text: &str) -> Vec<u8> {
    let mut bytes = BOM_BYTES.to_vec();
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// The real file's shape: BOM, LF, 4-space indent, override on line 3.
pub(crate) fn fixture(active: &str, dir: &str) -> Vec<u8> {
    with_bom(&settings_text(&override_line(active, dir), "\n"))
}

fn state(active: bool, dir: Option<&str>) -> ModsOverride {
    ModsOverride {
        active,
        directory: dir.map(PathBuf::from),
    }
}

fn text_of(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

/// `(common prefix, common suffix)` lengths; the suffix never overlaps the prefix.
fn common(a: &[u8], b: &[u8]) -> (usize, usize) {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_suffix)
        .take_while(|(x, y)| x == y)
        .count();
    (prefix, suffix)
}

/// The changed region of `original` lies inside `allowed`.
fn assert_confined(original: &[u8], changed: &[u8], allowed: Range<usize>) {
    let (prefix, suffix) = common(original, changed);
    assert!(
        prefix >= allowed.start && original.len() - suffix <= allowed.end,
        "changed bytes {}..{} are outside {allowed:?}",
        prefix,
        original.len() - suffix
    );
}

/// No error text may contain the fake password.
pub(crate) fn assert_no_secret(text: &str) {
    assert!(!text.contains(SECRET), "error text leaks file contents");
}

fn assert_op_error_clean(e: &OpError) {
    assert_no_secret(&e.to_string());
    assert_no_secret(&format!("{e:?}"));
    let api = ApiError::from(match e {
        OpError::WriteFailed(d) => OpError::WriteFailed(d.clone()),
        OpError::GameRunning => OpError::GameRunning,
        OpError::ServerRunning => OpError::ServerRunning,
        OpError::SettingsMissing => OpError::SettingsMissing,
        OpError::SettingsUnreadable => OpError::SettingsUnreadable,
        OpError::SettingsChanged => OpError::SettingsChanged,
        OpError::NoBackup => OpError::NoBackup,
    });
    assert_no_secret(&api.to_string());
    assert_no_secret(&format!("{api:?}"));
}

fn idle() -> Running {
    Running::default()
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------- reading

#[test]
fn read_override_reads_the_element_anywhere() {
    let bytes = fixture("true", TESTING_DIR);
    assert_eq!(
        read_override(&bytes).unwrap(),
        state(true, Some(TESTING_DIR))
    );

    let nested = "<root><a><modsDirectoryOverride active=\"1\" directory=\"X:\\m\"/></a></root>";
    assert_eq!(
        read_override(nested.as_bytes()).unwrap(),
        state(true, Some(r"X:\m"))
    );
}

#[test]
fn read_override_active_rules_and_missing_element() {
    let read = |attrs: &str| {
        let xml = format!("<r><modsDirectoryOverride {attrs}/></r>");
        read_override(xml.as_bytes()).unwrap()
    };
    assert!(read("active=\" TRUE \" directory=\"a\"").active);
    assert!(read("active=\"1\" directory=\"a\"").active);
    assert!(!read("active=\"false\" directory=\"a\"").active);
    assert!(!read("active=\"yes\" directory=\"a\"").active);
    assert!(!read("directory=\"a\"").active);
    assert_eq!(read("active=\"true\" directory=\"  \"").directory, None);

    assert_eq!(
        read_override(b"<gameSettings></gameSettings>").unwrap(),
        ModsOverride::default()
    );
}

#[test]
fn read_override_refuses_bad_input() {
    assert!(matches!(
        read_override(&[0xFF, 0xFE, b'<', 0]),
        Err(OpError::SettingsUnreadable)
    ));
    assert!(matches!(
        read_override(b"<a><b></a>"),
        Err(OpError::SettingsUnreadable)
    ));
    assert!(matches!(
        read_override(b""),
        Err(OpError::SettingsUnreadable)
    ));
}

// ---------------------------------------------------------------- patch fidelity

#[test]
fn switching_to_the_default_changes_only_the_active_value() {
    let original = fixture("true", TESTING_DIR);
    let patched = patch_override(&original, &state(false, None)).unwrap();

    assert_eq!(patched, fixture("false", TESTING_DIR));
    let text = text_of(&original);
    let start = text.find("active=\"").unwrap() + "active=\"".len();
    assert_confined(&original, &patched, start..start + "true".len());
    assert_eq!(patched[..3], BOM_BYTES);
}

#[test]
fn switching_to_another_folder_changes_only_the_directory_value() {
    let other = r"D:\Users\Someone\Documents\My Games\FS\other";
    let original = fixture("true", TESTING_DIR);
    let patched = patch_override(&original, &state(true, Some(other))).unwrap();

    assert_eq!(patched, fixture("true", other));
    let text = text_of(&original);
    let start = text.find("directory=\"").unwrap() + "directory=\"".len();
    assert_confined(&original, &patched, start..start + TESTING_DIR.len());
}

#[test]
fn switching_from_the_default_to_a_folder_changes_both_values_only() {
    let original = fixture("false", TESTING_DIR);
    let other = r"D:\x\mods2";
    let patched = patch_override(&original, &state(true, Some(other))).unwrap();
    assert_eq!(patched, fixture("true", other));
    let text = text_of(&original);
    let start = text.find("active=\"").unwrap() + "active=\"".len();
    let end = text.find("\"/>").unwrap();
    assert_confined(&original, &patched, start..end);
}

#[test]
fn round_trip_between_a_folder_and_the_default_is_byte_identical() {
    let original = fixture("true", TESTING_DIR);
    let at_default = patch_override(&original, &state(false, None)).unwrap();
    assert_ne!(at_default, original);
    let back = patch_override(&at_default, &state(true, Some(TESTING_DIR))).unwrap();
    assert_eq!(back, original);

    // And through a second folder.
    let at_b = patch_override(&original, &state(true, Some(r"D:\b"))).unwrap();
    let back = patch_override(&at_b, &state(true, Some(TESTING_DIR))).unwrap();
    assert_eq!(back, original);
}

#[test]
fn crlf_line_endings_and_no_bom_are_preserved() {
    let text = settings_text(&override_line("true", TESTING_DIR), "\r\n");
    let original = with_bom(&text);
    let patched = patch_override(&original, &state(false, None)).unwrap();
    assert_eq!(
        patched,
        with_bom(&settings_text(&override_line("false", TESTING_DIR), "\r\n"))
    );
    assert!(patched.windows(2).any(|w| w == b"\r\n"));

    let no_bom = text.into_bytes();
    let patched = patch_override(&no_bom, &state(false, None)).unwrap();
    assert!(!patched.starts_with(&BOM_BYTES));
    assert_eq!(
        patched,
        settings_text(&override_line("false", TESTING_DIR), "\r\n").into_bytes()
    );
}

#[test]
fn single_quotes_and_attribute_order_are_kept() {
    let line = "    <modsDirectoryOverride directory='D:\\a' active='true'/>";
    let original = with_bom(&settings_text(line, "\n"));
    let patched = patch_override(&original, &state(false, Some(r"D:\b"))).unwrap();
    let expected = settings_text(
        "    <modsDirectoryOverride directory='D:\\b' active='false'/>",
        "\n",
    );
    assert_eq!(patched, with_bom(&expected));
}

#[test]
fn special_characters_in_a_path_are_escaped_and_read_back() {
    let path = r#"D:\Me & You\<a>\"q" it's"#;
    for line in [
        "    <modsDirectoryOverride active=\"true\" directory=\"D:\\a\"/>",
        "    <modsDirectoryOverride active='true' directory='D:\\a'/>",
    ] {
        let original = with_bom(&settings_text(line, "\n"));
        let patched = patch_override(&original, &state(true, Some(path))).unwrap();
        let text = text_of(&patched);
        assert!(text.contains("&amp;"));
        assert!(text.contains("&lt;a&gt;"));
        assert!(text.contains("&quot;q&quot;"));
        assert!(text.contains("it&apos;s"));
        assert_eq!(read_override(&patched).unwrap(), state(true, Some(path)));
        // Everything but the directory value is untouched.
        let start = text_of(&original).find("directory=").unwrap() + "directory=\"".len();
        assert_confined(&original, &patched, start..start + 4);
    }
}

#[test]
fn an_existing_entity_in_the_directory_round_trips_byte_identically() {
    let original = with_bom(&settings_text(
        &override_line("true", r"D:\Tom &amp; Jerry\mods"),
        "\n",
    ));
    assert_eq!(
        read_override(&original).unwrap(),
        state(true, Some(r"D:\Tom & Jerry\mods"))
    );

    let away = patch_override(&original, &state(true, Some(r"D:\b"))).unwrap();
    let back = patch_override(&away, &state(true, Some(r"D:\Tom & Jerry\mods"))).unwrap();
    assert_eq!(back, original);

    // Setting the value it already has is a no-op, whatever escaping the file used.
    let numeric = with_bom(&settings_text(
        &override_line("true", r"D:\Tom &#38; Jerry\mods"),
        "\n",
    ));
    let same = patch_override(&numeric, &state(true, Some(r"D:\Tom & Jerry\mods"))).unwrap();
    assert_eq!(same, numeric);
}

#[test]
fn a_missing_attribute_replaces_the_element_with_the_canonical_one() {
    let original = with_bom(&settings_text(
        "    <modsDirectoryOverride active=\"true\"/>",
        "\n",
    ));
    let patched = patch_override(&original, &state(true, Some(r"D:\b"))).unwrap();
    assert_eq!(
        patched,
        with_bom(&settings_text(&override_line("true", r"D:\b"), "\n"))
    );

    // `directory` present, `active` missing; the directory is kept when no new one is given.
    let original = with_bom(&settings_text(
        "    <modsDirectoryOverride directory=\"D:\\a&amp;b\"/>",
        "\n",
    ));
    let patched = patch_override(&original, &state(true, None)).unwrap();
    assert_eq!(
        patched,
        with_bom(&settings_text(&override_line("true", r"D:\a&amp;b"), "\n"))
    );
}

#[test]
fn a_missing_element_is_inserted_as_the_roots_first_child() {
    let body = settings_text("    <extra>1</extra>", "\n");
    let original = with_bom(&body);
    let patched = patch_override(&original, &state(true, Some(r"D:\b"))).unwrap();
    let expected = body.replacen(
        "<gameSettings>\n",
        &format!("<gameSettings>\n{}\n", override_line("true", r"D:\b")),
        1,
    );
    assert_eq!(patched, with_bom(&expected));
    assert_eq!(read_override(&patched).unwrap(), state(true, Some(r"D:\b")));

    // CRLF and a different indent are copied.
    let crlf =
        with_bom("<?xml version=\"1.0\"?>\r\n<gameSettings>\r\n\t<a>1</a>\r\n</gameSettings>\r\n");
    let patched = patch_override(&crlf, &state(true, Some(r"D:\b"))).unwrap();
    assert_eq!(
        text_of(&patched),
        "\u{feff}<?xml version=\"1.0\"?>\r\n<gameSettings>\r\n\t<modsDirectoryOverride active=\"true\" directory=\"D:\\b\"/>\r\n\t<a>1</a>\r\n</gameSettings>\r\n"
    );
}

#[test]
fn a_missing_element_in_an_empty_root_gets_its_own_lines() {
    for (input, expected) in [
        (
            "<gameSettings></gameSettings>",
            "<gameSettings>\n    <modsDirectoryOverride active=\"true\" directory=\"D:\\b\"/>\n</gameSettings>",
        ),
        (
            "<gameSettings/>",
            "<gameSettings>\n    <modsDirectoryOverride active=\"true\" directory=\"D:\\b\"/>\n</gameSettings>",
        ),
        (
            "<gameSettings><a/></gameSettings>",
            "<gameSettings><modsDirectoryOverride active=\"true\" directory=\"D:\\b\"/><a/></gameSettings>",
        ),
    ] {
        let patched = patch_override(input.as_bytes(), &state(true, Some(r"D:\b"))).unwrap();
        assert_eq!(text_of(&patched), expected);
    }
}

#[test]
fn a_missing_element_and_an_inactive_target_changes_nothing() {
    let original = with_bom(&settings_text("    <extra>1</extra>", "\n"));
    let patched = patch_override(&original, &state(false, None)).unwrap();
    assert_eq!(patched, original);
}

#[test]
fn a_no_op_patch_returns_identical_bytes() {
    let original = fixture("true", TESTING_DIR);
    assert_eq!(
        patch_override(&original, &state(true, Some(TESTING_DIR))).unwrap(),
        original
    );
    let at_default = fixture("false", TESTING_DIR);
    assert_eq!(
        patch_override(&at_default, &state(false, None)).unwrap(),
        at_default
    );
    // `1` already means active, so it stays.
    let one = with_bom(&settings_text(&override_line("1", TESTING_DIR), "\n"));
    assert_eq!(
        patch_override(&one, &state(true, Some(TESTING_DIR))).unwrap(),
        one
    );
}

#[test]
fn not_utf8_and_malformed_input_are_refused_without_leaking() {
    let mut not_utf8 = fixture("true", TESTING_DIR);
    not_utf8.push(0xFF);
    let e = patch_override(&not_utf8, &state(false, None)).unwrap_err();
    assert!(matches!(e, OpError::SettingsUnreadable));
    assert_op_error_clean(&e);

    // UTF-16 with a BOM is refused too.
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "<gameSettings/>".encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    assert!(matches!(
        patch_override(&utf16, &state(false, None)),
        Err(OpError::SettingsUnreadable)
    ));

    let text = settings_text(&override_line("true", TESTING_DIR), "\n");
    let malformed = with_bom(&text.replace("</multiplayer>", "</multiplayerX>"));
    let e = patch_override(&malformed, &state(false, None)).unwrap_err();
    assert!(matches!(e, OpError::SettingsUnreadable));
    assert_op_error_clean(&e);
    let e = read_override(&malformed).unwrap_err();
    assert_op_error_clean(&e);
}

// ---------------------------------------------------------------- backups

#[test]
fn stamps_sort_in_time_order_and_parse_back() {
    let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
    assert_eq!(stamp_of(t), "20231114T221320123Z");
    assert_eq!(time_of_stamp("20231114T221320123Z"), Some(t));
    assert_eq!(time_of_stamp("20231114T221320123"), None);
    assert_eq!(time_of_stamp("2023-11-14T22:13:20Z"), None);
    // A leap day and the epoch.
    assert_eq!(stamp_of(UNIX_EPOCH), "19700101T000000000Z");
    let leap = UNIX_EPOCH + Duration::from_secs(1_709_164_800);
    assert_eq!(stamp_of(leap), "20240229T000000000Z");
    assert_eq!(time_of_stamp("20240229T000000000Z"), Some(leap));
}

#[test]
fn backup_writes_a_named_copy_and_creates_the_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("new").join("backups");
    let bytes = fixture("true", TESTING_DIR);
    let name = backup(&bytes, &dir, KEEP_BACKUPS).unwrap();

    assert!(name.starts_with("gameSettings-") && name.ends_with("Z.xml"));
    assert_eq!(name.len(), "gameSettings-".len() + 19 + ".xml".len());
    assert_eq!(fs::read(dir.join(&name)).unwrap(), bytes);
    assert_eq!(names_in(&dir), vec![name]);
}

#[test]
fn rapid_backups_get_distinct_names_in_creation_order() {
    let tmp = tempfile::tempdir().unwrap();
    let mut made = Vec::new();
    for i in 0..30u8 {
        made.push(backup(&[i], tmp.path(), 100).unwrap());
    }
    let mut sorted = made.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted, made,
        "names must be unique and sort in creation order"
    );

    let (latest, _) = latest_backup(tmp.path()).unwrap();
    assert_eq!(latest.file_name().unwrap().to_str().unwrap(), made[29]);
    assert_eq!(fs::read(latest).unwrap(), vec![29u8]);
}

#[test]
fn backups_are_pruned_to_the_newest_and_other_files_are_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for other in [
        "notes.txt",
        "gameSettings.xml",
        "settings-2020.xml",
        "gameSettings-x.txt",
    ] {
        fs::write(dir.join(other), b"keep me").unwrap();
    }
    // Older than anything the clock will produce now.
    fs::write(dir.join("gameSettings-20200101T000000000Z.xml"), b"old").unwrap();
    fs::write(dir.join("gameSettings-20200102T000000000Z.xml"), b"old").unwrap();

    let mut made = Vec::new();
    for i in 0..5u8 {
        made.push(backup(&[i], dir, 3).unwrap());
    }
    let remaining: Vec<String> = names_in(dir)
        .into_iter()
        .filter(|n| n.starts_with("gameSettings-") && n.ends_with(".xml"))
        .collect();
    assert_eq!(remaining, made[2..].to_vec());
    for other in [
        "notes.txt",
        "gameSettings.xml",
        "settings-2020.xml",
        "gameSettings-x.txt",
    ] {
        assert_eq!(fs::read(dir.join(other)).unwrap(), b"keep me");
    }
}

#[test]
fn retention_of_twenty_keeps_the_newest_twenty() {
    let tmp = tempfile::tempdir().unwrap();
    let mut made = Vec::new();
    for i in 0..25u8 {
        made.push(backup(&[i], tmp.path(), KEEP_BACKUPS).unwrap());
    }
    assert_eq!(names_in(tmp.path()), made[5..].to_vec());
}

#[test]
fn latest_backup_is_the_newest_by_name_with_its_time() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(latest_backup(tmp.path()).is_none());
    assert!(latest_backup(&tmp.path().join("missing")).is_none());

    fs::write(
        tmp.path().join("gameSettings-20240101T000000000Z.xml"),
        b"a",
    )
    .unwrap();
    fs::write(
        tmp.path().join("gameSettings-20240301T120000500Z.xml"),
        b"b",
    )
    .unwrap();
    fs::write(
        tmp.path().join("gameSettings-20240201T000000000Z.xml"),
        b"c",
    )
    .unwrap();
    fs::write(tmp.path().join("zzz.xml"), b"d").unwrap();
    let (path, time) = latest_backup(tmp.path()).unwrap();
    assert_eq!(fs::read(path).unwrap(), b"b");
    assert_eq!(time_of_stamp("20240301T120000500Z"), Some(time));
}

// ---------------------------------------------------------------- guarded write

/// A settings file in a temp folder and the temp folder.
fn settings_file(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("gameSettings.xml");
    fs::write(&path, bytes).unwrap();
    (tmp, path)
}

fn only_the_settings_file(tmp: &tempfile::TempDir) {
    assert_eq!(names_in(tmp.path()), vec!["gameSettings.xml".to_string()]);
}

#[test]
fn write_settings_replaces_the_file_and_leaves_no_temp_file() {
    let original = fixture("true", TESTING_DIR);
    let (tmp, path) = settings_file(&original);
    let new = fixture("false", TESTING_DIR);
    write_settings(&path, &new, &original, &idle).unwrap();
    assert_eq!(fs::read(&path).unwrap(), new);
    only_the_settings_file(&tmp);
}

#[test]
fn a_running_game_or_server_is_refused_before_any_change() {
    let original = fixture("true", TESTING_DIR);
    let new = fixture("false", TESTING_DIR);
    for (running, game) in [
        (
            Running {
                game: true,
                server: false,
            },
            true,
        ),
        (
            Running {
                game: false,
                server: true,
            },
            false,
        ),
        (
            Running {
                game: true,
                server: true,
            },
            true,
        ),
    ] {
        let (tmp, path) = settings_file(&original);
        let e = write_settings(&path, &new, &original, &|| running).unwrap_err();
        if game {
            assert!(matches!(e, OpError::GameRunning), "{e:?}");
        } else {
            assert!(matches!(e, OpError::ServerRunning), "{e:?}");
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        only_the_settings_file(&tmp);
    }
}

#[test]
fn the_game_starting_during_the_write_is_caught_before_the_rename() {
    let original = fixture("true", TESTING_DIR);
    let new = fixture("false", TESTING_DIR);
    let (tmp, path) = settings_file(&original);

    let started = Cell::new(false);
    let running = || Running {
        game: started.get(),
        server: false,
    };
    let start_game = |_: &Path| started.set(true);
    let hooks = Hooks {
        before_rename: Some(&start_game),
        after_rename: None,
    };
    let e = write_settings_with(&path, &new, &original, &running, &hooks).unwrap_err();
    assert!(matches!(e, OpError::GameRunning), "{e:?}");
    assert_eq!(fs::read(&path).unwrap(), original);
    only_the_settings_file(&tmp);
}

#[test]
fn a_concurrent_change_before_the_rename_is_not_overwritten() {
    let original = fixture("true", TESTING_DIR);
    let new = fixture("false", TESTING_DIR);
    let concurrent = fixture("true", r"D:\written\by\the\game");
    let (tmp, path) = settings_file(&original);

    let change = |p: &Path| fs::write(p, &concurrent).unwrap();
    let hooks = Hooks {
        before_rename: Some(&change),
        after_rename: None,
    };
    let e = write_settings_with(&path, &new, &original, &idle, &hooks).unwrap_err();
    assert!(matches!(e, OpError::SettingsChanged), "{e:?}");
    assert_op_error_clean(&e);
    assert_eq!(fs::read(&path).unwrap(), concurrent);
    only_the_settings_file(&tmp);

    // The file vanishing counts as a change too.
    let (tmp, path) = settings_file(&original);
    let remove = |p: &Path| fs::remove_file(p).unwrap();
    let hooks = Hooks {
        before_rename: Some(&remove),
        after_rename: None,
    };
    let e = write_settings_with(&path, &new, &original, &idle, &hooks).unwrap_err();
    assert!(matches!(e, OpError::SettingsChanged), "{e:?}");
    assert!(names_in(tmp.path()).is_empty());
}

/// Clears the read-only flag when dropped so the temp folder can always be deleted.
struct ReadOnly(PathBuf);

impl ReadOnly {
    fn set(path: &Path) -> Self {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions).unwrap();
        Self(path.to_path_buf())
    }
}

impl Drop for ReadOnly {
    #[allow(clippy::permissions_set_readonly_false)]
    fn drop(&mut self) {
        if let Ok(meta) = fs::metadata(&self.0) {
            let mut permissions = meta.permissions();
            permissions.set_readonly(false);
            let _ = fs::set_permissions(&self.0, permissions);
        }
    }
}

#[test]
fn a_read_only_file_fails_cleanly_and_stays_read_only() {
    let original = fixture("true", TESTING_DIR);
    let new = fixture("false", TESTING_DIR);
    let (tmp, path) = settings_file(&original);
    let guard = ReadOnly::set(&path);

    let result = write_settings(&path, &new, &original, &idle);
    let still_read_only = fs::metadata(&path).unwrap().permissions().readonly();
    let contents = fs::read(&path).unwrap();
    let names = names_in(tmp.path());
    drop(guard);

    let e = result.unwrap_err();
    assert!(matches!(e, OpError::WriteFailed(_)), "{e:?}");
    assert_op_error_clean(&e);
    assert!(still_read_only, "the read-only flag must never be cleared");
    assert_eq!(contents, original);
    assert_eq!(names, vec!["gameSettings.xml".to_string()]);
}

#[test]
fn a_failed_verification_puts_the_original_back() {
    let original = fixture("true", TESTING_DIR);
    let new = fixture("false", TESTING_DIR);
    let (tmp, path) = settings_file(&original);

    let corrupt = |p: &Path| fs::write(p, b"garbage").unwrap();
    let hooks = Hooks {
        before_rename: None,
        after_rename: Some(&corrupt),
    };
    let e = write_settings_with(&path, &new, &original, &idle, &hooks).unwrap_err();
    assert!(
        matches!(&e, OpError::WriteFailed(d) if d == "verify"),
        "{e:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    only_the_settings_file(&tmp);
}

#[test]
fn a_missing_settings_file_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("gameSettings.xml");
    let e = write_settings(&path, b"x", b"y", &idle).unwrap_err();
    assert!(matches!(e, OpError::SettingsMissing));
    assert!(names_in(tmp.path()).is_empty());
}

#[test]
fn temp_files_have_unique_names_next_to_the_original() {
    let (tmp, path) = settings_file(b"x");
    let a = stage_temp(&path, b"a").unwrap();
    let b = stage_temp(&path, b"b").unwrap();
    assert_ne!(a.0, b.0);
    assert_eq!(a.0.parent(), path.parent());
    let name = a.0.file_name().unwrap().to_str().unwrap().to_string();
    assert!(name.starts_with("gameSettings.xml.fsmm-") && name.ends_with(".tmp"));
    drop((a, b));
    only_the_settings_file(&tmp);
}
