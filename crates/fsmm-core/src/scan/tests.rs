//! Tests for enumerate, read_mod and scan_folder against fixtures generated in temp dirs.

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::*;
use crate::container::IconKind;
use crate::model::{Dependency, IssueKind, ModFormat, ModKind, Severity};
use crate::testutil::{write_folder, write_zip, Method};

fn xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n\
         <modDesc descVersion=\"105\">{body}</modDesc>"
    )
}

/// Writes `<name>.zip` with a `modDesc.xml` and extra files, then reads it back.
fn read_zip(desc: &[u8], extra: &[(&str, &[u8])], languages: &[&str]) -> ModRecord {
    let dir = tempfile::tempdir().unwrap();
    let mut files: Vec<(&str, &[u8])> = vec![("modDesc.xml", desc)];
    files.extend_from_slice(extra);
    write_zip(&dir.path().join("FS25_Test.zip"), &files, Method::Deflated);
    read_named(dir.path(), "FS25_Test.zip", languages)
}

fn read_named(dir: &Path, name: &str, languages: &[&str]) -> ModRecord {
    let (entries, _) = enumerate(dir).unwrap();
    let entry = entries.iter().find(|e| e.name == name).unwrap();
    read_mod(entry, "profile-1", languages)
}

fn read_body(body: &str, languages: &[&str]) -> ModRecord {
    read_zip(xml(body).as_bytes(), &[], languages)
}

fn issue_kinds(r: &ModRecord) -> Vec<(IssueKind, Severity, Option<&str>)> {
    r.entry
        .issues
        .iter()
        .map(|i| (i.kind, i.severity, i.detail.as_deref()))
        .collect()
}

// ---------------------------------------------------------------- read_mod: basics

#[test]
fn a_plain_mod_fills_the_entry() {
    for method in [Method::Stored, Method::Deflated] {
        let dir = tempfile::tempdir().unwrap();
        let desc = xml(
            "<title><en>Tractor</en></title><author>Someone</author><version>1.2.0.0</version>\
             <description><en>Drives.</en></description>",
        );
        let path = dir.path().join("FS25_Tractor.zip");
        write_zip(&path, &[("modDesc.xml", desc.as_bytes())], method);

        let r = read_named(dir.path(), "FS25_Tractor.zip", &[]);
        let e = &r.entry;
        assert_eq!(r.path, path);
        assert_eq!(e.id, "FS25_Tractor.zip");
        assert_eq!(e.file_name, "FS25_Tractor.zip");
        assert_eq!(e.mod_name, "FS25_Tractor");
        assert_eq!(e.profile_id, "profile-1");
        assert_eq!(e.format, ModFormat::Zip);
        assert_eq!(e.title, "Tractor");
        assert_eq!(e.author, "Someone");
        assert_eq!(e.version, "1.2.0.0");
        assert_eq!(e.description, "Drives.");
        assert_eq!(e.desc_version, Some(105));
        assert_eq!(e.size_bytes, fs::metadata(&path).unwrap().len());
        assert_eq!(e.icon, None);
        assert_eq!(e.source, ModSource::Unknown);
        assert_eq!(e.update, UpdateState::Unknown);
        assert!(e.also_in.is_empty());
        assert!(e.issues.is_empty());
        assert_eq!(r.icon, None);
    }
}

#[test]
fn modified_is_rfc3339_utc_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Time.zip");
    write_zip(
        &path,
        &[("modDesc.xml", xml("").as_bytes())],
        Method::Stored,
    );
    set_mtime(&path, 1_700_000_000);
    let r = read_named(dir.path(), "FS25_Time.zip", &[]);
    assert_eq!(r.entry.modified, "2023-11-14T22:13:20Z");
}

#[test]
fn rfc3339_clamps_times_it_cannot_format() {
    assert_eq!(
        rfc3339(UNIX_EPOCH - Duration::from_secs(10)),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(
        rfc3339(UNIX_EPOCH + Duration::from_secs(500_000_000_000)),
        "9999-12-31T23:59:59Z"
    );
}

#[test]
fn uppercase_zip_extension_is_removed_from_the_mod_name() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("FS25_Upper.ZIP"),
        &[("modDesc.xml", xml("").as_bytes())],
        Method::Stored,
    );
    let r = read_named(dir.path(), "FS25_Upper.ZIP", &[]);
    assert_eq!(r.entry.mod_name, "FS25_Upper");
    assert_eq!(r.entry.format, ModFormat::Zip);
}

#[test]
fn a_folder_mod_reads_like_a_zip() {
    let dir = tempfile::tempdir().unwrap();
    let desc = xml("<title>Folder mod</title><iconFilename>icons/i.png</iconFilename>");
    write_folder(
        &dir.path().join("FS25_Folder"),
        &[("modDesc.xml", desc.as_bytes()), ("icons/i.dds", b"DDS ")],
    );
    let r = read_named(dir.path(), "FS25_Folder", &[]);
    assert_eq!(r.entry.format, ModFormat::Folder);
    assert_eq!(r.entry.mod_name, "FS25_Folder");
    assert_eq!(r.entry.title, "Folder mod");
    assert_eq!(r.entry.size_bytes, desc.len() as u64 + 4);
    assert_eq!(
        r.icon,
        Some(IconLocation {
            rel: "icons/i.dds".into(),
            kind: IconKind::Dds
        })
    );
}

// ---------------------------------------------------------------- read_mod: XML shapes

#[test]
fn bom_missing_declaration_and_leading_comments_are_accepted() {
    let body = "<title>T</title><author>A</author>";
    let plain = format!("<modDesc descVersion=\"99\">{body}</modDesc>");
    let variants: Vec<Vec<u8>> = vec![
        [b"\xEF\xBB\xBF".as_slice(), xml(body).as_bytes()].concat(),
        plain.clone().into_bytes(),
        [b"\xEF\xBB\xBF".as_slice(), plain.as_bytes()].concat(),
        format!("<?xml version=\"1.0\"?>\n<!-- one -->\n<!-- two -->\n{plain}").into_bytes(),
        format!("\r\n  {plain}\r\n").into_bytes(),
    ];
    for (i, desc) in variants.iter().enumerate() {
        let r = read_zip(desc, &[], &[]);
        assert_eq!(r.entry.title, "T", "variant {i}");
        assert_eq!(r.entry.author, "A", "variant {i}");
        assert!(r.entry.issues.is_empty(), "variant {i}");
    }
}

#[test]
fn cdata_and_surrounding_whitespace_are_handled() {
    let r = read_body(
        "<title>\n  <en><![CDATA[ Big <b>Tractor</b> & Co ]]></en>\n</title>\
         <description><en><![CDATA[Line one\r\nLine two]]></en></description>\
         <author><![CDATA[ A & B ]]></author>",
        &[],
    );
    assert_eq!(r.entry.title, "Big <b>Tractor</b> & Co");
    assert_eq!(r.entry.description, "Line one\nLine two");
    assert_eq!(r.entry.author, "A & B");
}

#[test]
fn a_bare_ampersand_is_retried_and_still_loads() {
    let r = read_body(
        "<title><en>Tom & Jerry &amp; Co</en></title><l10n><text name=\"x\"><en>A & B</en></text></l10n>",
        &[],
    );
    assert!(r.entry.issues.is_empty());
    assert_eq!(r.entry.title, "Tom & Jerry & Co");
}

#[test]
fn an_invalid_mod_desc_is_not_a_mod() {
    let broken: [&[u8]; 4] = [
        b"<modDesc descVersion=\"105\"><title>never closed</modDesc",
        b"this is not xml",
        b"<other><title>x</title></other>",
        b"",
    ];
    for desc in broken {
        let r = read_zip(desc, &[], &[]);
        assert_eq!(
            issue_kinds(&r),
            [(IssueKind::NotAMod, Severity::Error, Some("invalidModDesc"))]
        );
        // The record still exists and is named after the file.
        assert_eq!(r.entry.title, "FS25_Test");
        assert_eq!(r.entry.kind, ModKind::Other);
    }
}

#[test]
fn a_mod_desc_over_the_size_cap_is_rejected() {
    let mut big = xml("").into_bytes();
    big.extend(std::iter::repeat_n(b' ', 5 * 1024 * 1024));
    for method in [Method::Stored, Method::Deflated] {
        let dir = tempfile::tempdir().unwrap();
        write_zip(
            &dir.path().join("FS25_Big.zip"),
            &[("modDesc.xml", &big)],
            method,
        );
        let r = read_named(dir.path(), "FS25_Big.zip", &[]);
        assert_eq!(
            issue_kinds(&r),
            [(IssueKind::NotAMod, Severity::Error, Some("invalidModDesc"))]
        );
    }
}

// ---------------------------------------------------------------- read_mod: text fields

#[test]
fn language_choice_prefers_the_requested_order_then_en_then_the_first() {
    let body = "<title><de>Traktor</de><en>Tractor</en><fr>Tracteur</fr></title>\
                <description><de>Beschreibung</de><fr>Description</fr></description>";
    let title = |langs: &[&str]| read_body(body, langs).entry.title;
    let description = |langs: &[&str]| read_body(body, langs).entry.description;

    assert_eq!(title(&[]), "Tractor");
    assert_eq!(title(&["de"]), "Traktor");
    assert_eq!(title(&["xx", "fr", "de"]), "Tracteur");
    assert_eq!(title(&["xx"]), "Tractor");
    // No `en`: the first child when no preferred language matches.
    assert_eq!(description(&[]), "Beschreibung");
    assert_eq!(description(&["fr"]), "Description");
    assert_eq!(description(&["xx"]), "Beschreibung");
}

#[test]
fn language_codes_match_exactly_without_aliasing() {
    let body = "<title><cs>Chinese</cs><cz>Czech</cz><en>English</en></title>";
    assert_eq!(read_body(body, &["cz"]).entry.title, "Czech");
    assert_eq!(read_body(body, &["cs"]).entry.title, "Chinese");
    assert_eq!(read_body(body, &["uk"]).entry.title, "English");
}

#[test]
fn an_empty_preferred_language_falls_through() {
    let body = "<title><de>  </de><en>Tractor</en></title>";
    assert_eq!(read_body(body, &["de"]).entry.title, "Tractor");
}

#[test]
fn plain_text_title_and_description_are_used_as_is() {
    let r = read_body(
        "<title>  Plain title </title><description>Plain\r\ntext</description>",
        &["de"],
    );
    assert_eq!(r.entry.title, "Plain title");
    assert_eq!(r.entry.description, "Plain\ntext");
}

#[test]
fn missing_fields_default_and_title_falls_back_to_the_mod_name() {
    let r = read_body("", &[]);
    let e = &r.entry;
    assert_eq!(e.title, "FS25_Test");
    assert_eq!(e.author, "");
    assert_eq!(e.version, "");
    assert_eq!(e.description, "");
    assert!(!e.multiplayer);
    assert!(e.dependencies.is_empty());
    assert_eq!(e.kind, ModKind::Other);
    assert!(e.issues.is_empty());

    let r = read_body("<title><en></en></title>", &[]);
    assert_eq!(r.entry.title, "FS25_Test");
    let r = read_body("<title>   </title>", &[]);
    assert_eq!(r.entry.title, "FS25_Test");
}

#[test]
fn version_strings_are_kept_as_written() {
    for v in ["1.0.0.0", "1.0", "2.1.0", "0.9.5.1-beta"] {
        let r = read_body(&format!("<version>{v}</version>"), &[]);
        assert_eq!(r.entry.version, v);
    }
}

#[test]
fn desc_version_comes_from_the_root_attribute() {
    let desc_version = |root: &str| {
        let desc = format!("<modDesc {root}></modDesc>");
        read_zip(desc.as_bytes(), &[], &[]).entry.desc_version
    };
    assert_eq!(desc_version("descVersion=\"116\""), Some(116));
    assert_eq!(desc_version("descVersion=\" 90 \""), Some(90));
    assert_eq!(desc_version("descVersion=\"abc\""), None);
    assert_eq!(desc_version("descVersion=\"-1\""), None);
    assert_eq!(desc_version("descVersion=\"1.5\""), None);
    assert_eq!(desc_version(""), None);
}

#[test]
fn multiplayer_is_read_from_the_supported_attribute() {
    let mp = |body: &str| read_body(body, &[]).entry.multiplayer;
    assert!(mp("<multiplayer supported=\"true\"/>"));
    assert!(mp("<multiplayer supported=\"True\"/>"));
    assert!(!mp("<multiplayer supported=\"false\"/>"));
    assert!(!mp("<multiplayer/>"));
    assert!(!mp(""));
}

#[test]
fn dependencies_are_listed_unsatisfied_until_assessed() {
    let r = read_body(
        "<dependencies><dependency>FS25_A</dependency><dependency> FS25_B </dependency>\
         <dependency/></dependencies>",
        &[],
    );
    assert_eq!(
        r.entry.dependencies,
        [
            Dependency {
                mod_name: "FS25_A".into(),
                satisfied: false
            },
            Dependency {
                mod_name: "FS25_B".into(),
                satisfied: false
            },
        ]
    );
    assert!(r.entry.issues.is_empty());
}

// ---------------------------------------------------------------- read_mod: kind

const VEHICLE: &[u8] = b"<?xml version=\"1.0\"?>\n<!-- v -->\n<vehicle type=\"t\"/>";
const PLACEABLE: &[u8] = b"\xEF\xBB\xBF<placeable type=\"t\"/>";

fn store_items(paths: &[&str]) -> String {
    let items: String = paths
        .iter()
        .map(|p| format!("<storeItem xmlFilename=\"{p}\"/>"))
        .collect();
    format!("<storeItems>{items}</storeItems>")
}

fn kind_of(body: &str, extra: &[(&str, &[u8])]) -> ModKind {
    read_zip(xml(body).as_bytes(), extra, &[]).entry.kind
}

#[test]
fn maps_win_over_everything() {
    let body = format!(
        "<maps><map id=\"m\" configFilename=\"map.xml\"/></maps>{}<extraSourceFiles/>",
        store_items(&["a.xml"])
    );
    assert_eq!(kind_of(&body, &[("a.xml", VEHICLE)]), ModKind::Map);
    // An empty <maps> element is not a map.
    assert_eq!(kind_of("<maps/>", &[]), ModKind::Other);
}

#[test]
fn store_items_decide_vehicle_or_placeable_by_majority() {
    let files: &[(&str, &[u8])] = &[
        ("v1.xml", VEHICLE),
        ("v2.xml", VEHICLE),
        ("p1.xml", PLACEABLE),
    ];
    let body = store_items(&["v1.xml", "v2.xml", "p1.xml"]);
    assert_eq!(kind_of(&body, files), ModKind::Vehicle);
    // A tie and any other mix with a placeable count as placeable.
    let body = store_items(&["v1.xml", "p1.xml"]);
    assert_eq!(kind_of(&body, files), ModKind::Placeable);
    let body = store_items(&["v1.xml", "p1.xml", "p1.xml"]);
    assert_eq!(kind_of(&body, files), ModKind::Placeable);
    let body = store_items(&["p1.xml"]);
    assert_eq!(kind_of(&body, files), ModKind::Placeable);
}

#[test]
fn store_item_paths_resolve_case_insensitively_and_skip_base_game_paths() {
    let body = store_items(&[
        "$data/vehicles/tractor.xml",
        "$DATA/vehicles/other.xml",
        "Vehicles\\Mine.XML",
        "missing.xml",
    ]);
    // Only the resolvable, in-mod item counts.
    assert_eq!(
        kind_of(&body, &[("vehicles/mine.xml", VEHICLE)]),
        ModKind::Vehicle
    );
    // Base game items alone say nothing about the kind.
    let body = store_items(&["$data/vehicles/tractor.xml"]);
    assert_eq!(kind_of(&body, &[]), ModKind::Other);
}

#[test]
fn only_the_first_fifty_store_items_are_sampled() {
    let mut paths: Vec<String> = (0..50).map(|i| format!("missing{i}.xml")).collect();
    paths.push("late.xml".into());
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    // The only readable item is number 51, which is past the sample.
    assert_eq!(
        kind_of(&store_items(&refs), &[("late.xml", PLACEABLE)]),
        ModKind::Other
    );
}

#[test]
fn script_mods_have_extra_source_files_or_specializations_only() {
    assert_eq!(
        kind_of(
            "<extraSourceFiles><sourceFile filename=\"a.lua\"/></extraSourceFiles>",
            &[]
        ),
        ModKind::Script
    );
    assert_eq!(
        kind_of(
            "<specializations><specialization name=\"x\"/></specializations>",
            &[]
        ),
        ModKind::Script
    );
    // Store items take precedence over the script markers.
    let body = format!("<extraSourceFiles/>{}", store_items(&["v.xml"]));
    assert_eq!(kind_of(&body, &[("v.xml", VEHICLE)]), ModKind::Vehicle);
    assert_eq!(kind_of("<title>x</title>", &[]), ModKind::Other);
}

// ---------------------------------------------------------------- read_mod: icon

fn icon_of(body: &str, extra: &[(&str, &[u8])]) -> Option<IconLocation> {
    let r = read_zip(xml(body).as_bytes(), extra, &[]);
    assert!(r.entry.issues.is_empty());
    assert_eq!(r.entry.icon, None, "the URL is filled by the app layer");
    r.icon
}

fn loc(rel: &str, kind: IconKind) -> Option<IconLocation> {
    Some(IconLocation {
        rel: rel.into(),
        kind,
    })
}

#[test]
fn icon_exact_match() {
    let body = "<iconFilename>icon.png</iconFilename>";
    assert_eq!(
        icon_of(body, &[("icon.png", b"x")]),
        loc("icon.png", IconKind::Png)
    );
    let body = "<iconFilename>icon.dds</iconFilename>";
    assert_eq!(
        icon_of(body, &[("icon.dds", b"x")]),
        loc("icon.dds", IconKind::Dds)
    );
}

#[test]
fn icon_prefers_the_declared_file_over_the_swapped_one() {
    let body = "<iconFilename>icon.png</iconFilename>";
    assert_eq!(
        icon_of(body, &[("icon.dds", b"x"), ("icon.png", b"x")]),
        loc("icon.png", IconKind::Png)
    );
}

#[test]
fn icon_extension_is_swapped_when_the_declared_file_is_missing() {
    assert_eq!(
        icon_of(
            "<iconFilename>icon.png</iconFilename>",
            &[("icon.dds", b"x")]
        ),
        loc("icon.dds", IconKind::Dds)
    );
    assert_eq!(
        icon_of(
            "<iconFilename>icon.DDS</iconFilename>",
            &[("icon.png", b"x")]
        ),
        loc("icon.png", IconKind::Png)
    );
}

#[test]
fn icon_in_a_subfolder_is_found_case_insensitively() {
    assert_eq!(
        icon_of(
            "<iconFilename>store/Icon.png</iconFilename>",
            &[("Store/icon.dds", b"x")]
        ),
        loc("Store/icon.dds", IconKind::Dds)
    );
}

#[test]
fn a_missing_or_unusable_icon_is_none_without_an_issue() {
    assert_eq!(icon_of("<iconFilename>icon.png</iconFilename>", &[]), None);
    assert_eq!(icon_of("", &[("icon.dds", b"x")]), None);
    assert_eq!(icon_of("<iconFilename/>", &[("icon.dds", b"x")]), None);
    // Neither png nor dds: not something the icon decoder can read.
    assert_eq!(
        icon_of(
            "<iconFilename>icon.jpg</iconFilename>",
            &[("icon.jpg", b"x")]
        ),
        None
    );
}

// ---------------------------------------------------------------- read_mod: not a mod

#[test]
fn a_zip_without_mod_desc_is_not_a_mod() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("FS25_Empty.zip"),
        &[("readme.txt", b"hi"), ("sub/other.xml", b"<a/>")],
        Method::Stored,
    );
    let r = read_named(dir.path(), "FS25_Empty.zip", &[]);
    assert_eq!(
        issue_kinds(&r),
        [(IssueKind::NotAMod, Severity::Error, Some("noModDesc"))]
    );
}

#[test]
fn a_folder_without_mod_desc_is_not_a_mod() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("_staged")).unwrap();
    let r = read_named(dir.path(), "_staged", &[]);
    assert_eq!(r.entry.format, ModFormat::Folder);
    assert_eq!(
        issue_kinds(&r),
        [(IssueKind::NotAMod, Severity::Error, Some("noModDesc"))]
    );
}

#[test]
fn a_zip_holding_another_zip_is_a_nested_archive() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("FS25_Outer.zip"),
        &[("pack/FS25_Inner.zip", b"PK"), ("readme.txt", b"hi")],
        Method::Deflated,
    );
    let r = read_named(dir.path(), "FS25_Outer.zip", &[]);
    assert_eq!(
        issue_kinds(&r),
        [(
            IssueKind::NestedArchive,
            Severity::Warning,
            Some("pack/FS25_Inner.zip")
        )]
    );
}

#[test]
fn a_wrapper_folder_is_a_nested_archive() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("FS25_Wrapped.zip"),
        &[
            ("FS25_Wrapped/", b""),
            ("FS25_Wrapped/modDesc.xml", xml("").as_bytes()),
        ],
        Method::Deflated,
    );
    let r = read_named(dir.path(), "FS25_Wrapped.zip", &[]);
    assert_eq!(
        issue_kinds(&r),
        [(
            IssueKind::NestedArchive,
            Severity::Warning,
            Some("FS25_Wrapped/modDesc.xml")
        )]
    );

    // Two folders deep is not a wrapper.
    write_zip(
        &dir.path().join("FS25_Deep.zip"),
        &[("a/b/modDesc.xml", xml("").as_bytes())],
        Method::Deflated,
    );
    let r = read_named(dir.path(), "FS25_Deep.zip", &[]);
    assert_eq!(
        issue_kinds(&r),
        [(IssueKind::NotAMod, Severity::Error, Some("noModDesc"))]
    );
}

#[test]
fn an_unreadable_zip_is_not_a_mod() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("FS25_Broken.zip"), b"definitely not a zip").unwrap();
    fs::write(dir.path().join("FS25_Zero.zip"), b"").unwrap();
    for name in ["FS25_Broken.zip", "FS25_Zero.zip"] {
        let r = read_named(dir.path(), name, &[]);
        assert_eq!(
            issue_kinds(&r),
            [(
                IssueKind::NotAMod,
                Severity::Error,
                Some("unreadableArchive")
            )],
            "{name}"
        );
        assert_eq!(r.entry.mod_name, name.trim_end_matches(".zip"));
    }
}

// ---------------------------------------------------------------- enumerate

fn set_mtime(path: &Path, secs: u64) {
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::from_secs(secs))
        .unwrap();
}

#[test]
fn enumerate_lists_zips_and_folders_sorted_case_insensitively() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["b_mod.zip", "A_mod.zip", "C_mod.ZIP"] {
        fs::write(dir.path().join(name), b"zip").unwrap();
    }
    write_folder(&dir.path().join("a_folder"), &[("modDesc.xml", b"x")]);
    // Direct children only: nested zips are not listed.
    write_folder(&dir.path().join("B_folder"), &[("inner.zip", b"x")]);

    let (entries, skipped) = enumerate(dir.path()).unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "a_folder",
            "A_mod.zip",
            "B_folder",
            "b_mod.zip",
            "C_mod.ZIP"
        ]
    );
    let formats: Vec<_> = entries.iter().map(|e| e.format).collect();
    assert_eq!(
        formats,
        [
            ModFormat::Folder,
            ModFormat::Zip,
            ModFormat::Folder,
            ModFormat::Zip,
            ModFormat::Zip
        ]
    );
    assert_eq!(entries[1].size, 3);
    assert!(skipped.is_empty());
}

#[test]
fn enumerate_ignores_dot_entries_and_skips_other_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".stfolder")).unwrap();
    fs::create_dir(dir.path().join(".stversions")).unwrap();
    fs::write(dir.path().join(".stignore"), b"x").unwrap();
    fs::write(dir.path().join(".hidden.zip"), b"x").unwrap();
    fs::write(dir.path().join("notes.txt"), b"x").unwrap();
    fs::write(dir.path().join("noextension"), b"x").unwrap();
    fs::write(dir.path().join("FS25_Real.zip"), b"x").unwrap();
    fs::create_dir(dir.path().join("_staged")).unwrap();

    let (entries, skipped) = enumerate(dir.path()).unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["_staged", "FS25_Real.zip"]);
    assert_eq!(skipped.len(), 2);
    assert!(skipped.iter().all(|s| s.reason == SkipReason::NotAZip));
    let mut skipped_names: Vec<_> = skipped
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    skipped_names.sort();
    assert_eq!(skipped_names, ["noextension", "notes.txt"]);
}

#[test]
fn enumerate_fails_for_a_missing_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(enumerate(&dir.path().join("nope")).is_err());
}

#[cfg(windows)]
#[test]
fn enumerate_ignores_hidden_and_system_entries() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    for name in ["hidden.zip", "system.zip", "visible.zip"] {
        fs::write(dir.path().join(name), b"x").unwrap();
    }
    fs::create_dir(dir.path().join("hiddenfolder")).unwrap();
    for (flag, name) in [
        ("+h", "hidden.zip"),
        ("+s", "system.zip"),
        ("+h", "hiddenfolder"),
    ] {
        let status = Command::new("attrib")
            .arg(flag)
            .arg(dir.path().join(name))
            .status()
            .unwrap();
        assert!(status.success());
    }
    let (entries, skipped) = enumerate(dir.path()).unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["visible.zip"]);
    assert!(skipped.is_empty());
}

#[test]
fn folder_entries_report_total_size_and_newest_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("FS25_Folder");
    write_folder(
        &root,
        &[
            ("modDesc.xml", &[0u8; 10]),
            ("a/b/c.dds", &[0u8; 100]),
            ("a/d.xml", &[0u8; 1000]),
        ],
    );
    set_mtime(&root.join("modDesc.xml"), 1_000_000);
    set_mtime(&root.join("a/b/c.dds"), 3_000_000);
    set_mtime(&root.join("a/d.xml"), 2_000_000);

    let (entries, _) = enumerate(dir.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].size, 1110);
    assert_eq!(
        entries[0].modified,
        UNIX_EPOCH + Duration::from_secs(3_000_000)
    );
}

#[test]
fn an_empty_folder_uses_its_own_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("_staged");
    fs::create_dir(&root).unwrap();
    let (entries, _) = enumerate(dir.path()).unwrap();
    assert_eq!(entries[0].size, 0);
    assert_eq!(
        entries[0].modified,
        fs::metadata(&root).unwrap().modified().unwrap()
    );
}

#[test]
fn zip_entries_report_file_size_and_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Z.zip");
    fs::write(&path, [0u8; 42]).unwrap();
    set_mtime(&path, 1_234_567);
    let (entries, _) = enumerate(dir.path()).unwrap();
    assert_eq!(entries[0].size, 42);
    assert_eq!(
        entries[0].modified,
        UNIX_EPOCH + Duration::from_secs(1_234_567)
    );
}

// ---------------------------------------------------------------- scan_folder

#[test]
fn scan_folder_reads_assesses_and_summarises_a_mixed_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let vehicle_body = format!(
        "<title><en>Cart</en></title><iconFilename>icon.png</iconFilename>\
         <dependencies><dependency>FS25_Lib</dependency><dependency>FS25_Gone</dependency>\
         <dependency>pdlc_pack</dependency></dependencies>{}",
        store_items(&["cart.xml"])
    );
    write_zip(
        &root.join("FS25_Cart.zip"),
        &[
            (
                "modDesc.xml",
                xml(&vehicle_body).replace("105", "120").as_bytes(),
            ),
            ("cart.xml", VEHICLE),
            ("icon.dds", b"x"),
        ],
        Method::Deflated,
    );
    write_zip(
        &root.join("FS25_Lib.zip"),
        &[("modDesc.xml", xml("<extraSourceFiles/>").as_bytes())],
        Method::Stored,
    );
    write_zip(
        &root.join("FS25_Lib.sync-conflict-20261117-155914-UPKGIQY.zip"),
        &[("modDesc.xml", xml("").as_bytes())],
        Method::Stored,
    );
    // Twin: folder and zip.
    write_folder(
        &root.join("FS25_Twin"),
        &[(
            "modDesc.xml",
            xml("<maps><map id=\"a\"/></maps>").as_bytes(),
        )],
    );
    write_zip(
        &root.join("FS25_Twin.zip"),
        &[("modDesc.xml", xml("").as_bytes())],
        Method::Stored,
    );
    write_zip(
        &root.join("2BadName.zip"),
        &[("modDesc.xml", xml("").as_bytes())],
        Method::Stored,
    );
    fs::write(root.join("broken.zip"), b"nope").unwrap();
    fs::write(root.join("readme.txt"), b"hello").unwrap();
    fs::create_dir(root.join(".stfolder")).unwrap();

    let scan = scan_folder(root, "profile-9", &["de"], Some(110)).unwrap();

    let names: Vec<_> = scan
        .mods
        .iter()
        .map(|r| r.entry.file_name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "2BadName.zip",
            "broken.zip",
            "FS25_Cart.zip",
            "FS25_Lib.sync-conflict-20261117-155914-UPKGIQY.zip",
            "FS25_Lib.zip",
            "FS25_Twin",
            "FS25_Twin.zip",
        ]
    );
    assert!(scan.mods.iter().all(|r| r.entry.profile_id == "profile-9"));
    assert_eq!(scan.skipped.len(), 1);

    let by_name = |n: &str| scan.mods.iter().find(|r| r.entry.file_name == n).unwrap();

    let cart = by_name("FS25_Cart.zip");
    assert_eq!(cart.entry.title, "Cart");
    assert_eq!(cart.entry.kind, ModKind::Vehicle);
    assert_eq!(cart.icon.as_ref().map(|i| i.kind), Some(IconKind::Dds));
    assert_eq!(
        issue_kinds(cart),
        [
            (
                IssueKind::MissingDependency,
                Severity::Warning,
                Some("FS25_Gone")
            ),
            (
                IssueKind::DescVersionMismatch,
                Severity::Warning,
                Some("110")
            ),
        ]
    );
    let satisfied: Vec<_> = cart
        .entry
        .dependencies
        .iter()
        .map(|d| d.satisfied)
        .collect();
    assert_eq!(satisfied, [true, false, true]);

    assert_eq!(by_name("FS25_Lib.zip").entry.kind, ModKind::Script);
    assert_eq!(
        issue_kinds(by_name(
            "FS25_Lib.sync-conflict-20261117-155914-UPKGIQY.zip"
        ))[0],
        (IssueKind::ConflictCopy, Severity::Warning, Some("FS25_Lib"))
    );
    assert_eq!(
        issue_kinds(by_name("2BadName.zip"))[0],
        (IssueKind::InvalidFileName, Severity::Error, None)
    );
    assert_eq!(
        issue_kinds(by_name("broken.zip")),
        [(
            IssueKind::NotAMod,
            Severity::Error,
            Some("unreadableArchive")
        )]
    );
    assert_eq!(by_name("FS25_Twin").entry.kind, ModKind::Map);
    assert_eq!(
        issue_kinds(by_name("FS25_Twin"))[0],
        (
            IssueKind::ZipAndFolderTwin,
            Severity::Warning,
            Some("FS25_Twin.zip")
        )
    );
    assert_eq!(
        issue_kinds(by_name("FS25_Twin.zip"))[0],
        (
            IssueKind::ZipAndFolderTwin,
            Severity::Warning,
            Some("FS25_Twin")
        )
    );

    let s = &scan.summary;
    assert_eq!(s.profile_id, "profile-9");
    assert_eq!(s.mod_count, 7);
    assert_eq!(
        s.total_bytes,
        scan.mods.iter().map(|r| r.entry.size_bytes).sum::<u64>()
    );
    // Cart, conflict copy, bad name, broken, and both twins; the plain library is clean.
    assert_eq!(s.issue_count, 6);
    assert!(s.scanned_at.ends_with('Z'));
}

#[test]
fn scan_folder_on_an_empty_folder_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let scan = scan_folder(dir.path(), "p", &[], None).unwrap();
    assert!(scan.mods.is_empty());
    assert!(scan.skipped.is_empty());
    assert_eq!(scan.summary.mod_count, 0);
    assert_eq!(scan.summary.issue_count, 0);
    assert_eq!(scan.summary.total_bytes, 0);
}

#[test]
fn scan_folder_fails_for_a_missing_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan_folder(&dir.path().join("nope"), "p", &[], None).is_err());
}
