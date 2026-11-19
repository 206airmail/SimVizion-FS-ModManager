//! Reading `modDesc.xml` and the few other files needed to classify a mod.

use roxmltree::{Document, Node};

use super::{NOT_A_MOD_INVALID_MOD_DESC, NOT_A_MOD_UNREADABLE};
use crate::container::{IconKind, IconLocation, ModContainer, ReadError};
use crate::model::{Dependency, ModKind};

/// Largest `modDesc.xml` read. Real ones are a few KiB to a few hundred KiB.
const MOD_DESC_LIMIT: u64 = 4 * 1024 * 1024;
/// How many store item files are sampled to decide between vehicle and placeable.
const STORE_ITEM_SAMPLE: usize = 50;
/// The root element is within this many bytes of the start of a store item file.
const STORE_ITEM_HEAD: usize = 1024;

/// What `modDesc.xml` says about a mod. Title falls back to the mod name in the caller.
pub(super) struct Desc {
    pub title: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub desc_version: Option<u32>,
    pub multiplayer: bool,
    pub dependencies: Vec<Dependency>,
    pub kind: ModKind,
    pub icon: Option<IconLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DescError {
    /// The entry could not be read from the container.
    Unreadable,
    /// Too large, not XML (even after the `&` retry), or the root is not `modDesc`.
    Invalid,
}

impl DescError {
    pub(super) fn detail(self) -> &'static str {
        match self {
            DescError::Unreadable => NOT_A_MOD_UNREADABLE,
            DescError::Invalid => NOT_A_MOD_INVALID_MOD_DESC,
        }
    }
}

/// Reads the root `modDesc.xml` of `container` (the caller has checked that it exists).
pub(super) fn read_desc(
    container: &mut ModContainer,
    languages: &[&str],
) -> Result<Desc, DescError> {
    let bytes = container
        .read("modDesc.xml", MOD_DESC_LIMIT)
        .map_err(|e| match e {
            ReadError::TooLarge { .. } => DescError::Invalid,
            _ => DescError::Unreadable,
        })?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    // The game loads files with a bare `&` in text, which strict XML rejects (plan decision 3).
    let escaped;
    let doc = match Document::parse(text) {
        Ok(doc) => doc,
        Err(_) => {
            escaped = escape_bare_ampersands(text);
            Document::parse(&escaped).map_err(|_| DescError::Invalid)?
        }
    };
    let root = doc.root_element();
    if root.tag_name().name() != "modDesc" {
        return Err(DescError::Invalid);
    }

    let (kind, icon) = (classify(root, container), find_icon(root, container));
    Ok(Desc {
        title: localized(child(root, "title"), languages),
        author: child(root, "author").map(text_of).unwrap_or_default(),
        version: child(root, "version").map(text_of).unwrap_or_default(),
        description: localized(child(root, "description"), languages),
        desc_version: root
            .attribute("descVersion")
            .and_then(|v| v.trim().parse().ok()),
        multiplayer: child(root, "multiplayer")
            .and_then(|n| n.attribute("supported"))
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
        dependencies: dependencies(root),
        kind,
        icon,
    })
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// The element's own text (CDATA is merged into text nodes), trimmed, with `\n` line ends.
fn text_of(node: Node) -> String {
    let text: String = node
        .children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    text.trim().replace("\r\n", "\n")
}

/// A `<title>` or `<description>`: language child elements when present (first preferred
/// language that has text, then `en`, then the first child), otherwise the element's own text.
fn localized(node: Option<Node>, languages: &[&str]) -> String {
    let Some(node) = node else {
        return String::new();
    };
    let Some(first) = node.children().find(|n| n.is_element()) else {
        return text_of(node);
    };
    for lang in languages.iter().copied().chain(["en"]) {
        let picked = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == lang)
            .map(text_of)
            .find(|t| !t.is_empty());
        if let Some(text) = picked {
            return text;
        }
    }
    text_of(first)
}

fn dependencies(root: Node) -> Vec<Dependency> {
    let Some(list) = child(root, "dependencies") else {
        return Vec::new();
    };
    list.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "dependency")
        .map(text_of)
        .filter(|name| !name.is_empty())
        .map(|mod_name| Dependency {
            mod_name,
            satisfied: false,
        })
        .collect()
}

fn classify(root: Node, container: &mut ModContainer) -> ModKind {
    if child(root, "maps").is_some_and(|maps| child(maps, "map").is_some()) {
        return ModKind::Map;
    }

    let (mut vehicles, mut placeables) = (0usize, 0usize);
    let store_items = child(root, "storeItems")
        .into_iter()
        .flat_map(|items| items.children())
        .filter(|n| n.is_element() && n.tag_name().name() == "storeItem")
        .filter_map(|n| n.attribute("xmlFilename"))
        .filter(|path| !is_base_game_path(path))
        .take(STORE_ITEM_SAMPLE);
    for path in store_items {
        let Some(found) = container.find(path) else {
            continue;
        };
        let Ok(head) = container.read_head(&found, STORE_ITEM_HEAD) else {
            continue;
        };
        match root_element_name(&head).as_deref() {
            Some("vehicle") => vehicles += 1,
            Some("placeable") => placeables += 1,
            _ => {}
        }
    }
    if vehicles > placeables {
        ModKind::Vehicle
    } else if placeables > 0 {
        ModKind::Placeable
    } else if child(root, "extraSourceFiles").is_some() || child(root, "specializations").is_some()
    {
        ModKind::Script
    } else {
        ModKind::Other
    }
}

/// `$data/...` points into the base game, not into the mod.
fn is_base_game_path(path: &str) -> bool {
    path.trim()
        .get(..6)
        .is_some_and(|p| p.eq_ignore_ascii_case("$data/") || p.eq_ignore_ascii_case("$data\\"))
}

/// The root element name from the start of an XML file, skipping a BOM, whitespace, the
/// declaration, comments and a doctype.
fn root_element_name(head: &[u8]) -> Option<String> {
    let mut rest = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    loop {
        rest = rest.trim_ascii_start();
        if let Some(after) = rest.strip_prefix(b"<?") {
            rest = skip_past(after, b"?>")?;
        } else if let Some(after) = rest.strip_prefix(b"<!--") {
            rest = skip_past(after, b"-->")?;
        } else if let Some(after) = rest.strip_prefix(b"<!") {
            rest = skip_past(after, b">")?;
        } else {
            break;
        }
    }
    let name = rest.strip_prefix(b"<")?;
    let end = name
        .iter()
        .position(|b| b.is_ascii_whitespace() || matches!(b, b'>' | b'/'))
        .unwrap_or(name.len());
    let name = std::str::from_utf8(&name[..end]).ok()?;
    (!name.is_empty()).then(|| name.to_string())
}

fn skip_past<'a>(haystack: &'a [u8], marker: &[u8]) -> Option<&'a [u8]> {
    let at = haystack.windows(marker.len()).position(|w| w == marker)?;
    Some(&haystack[at + marker.len()..])
}

/// Resolves `<iconFilename>`. Mods often declare `icon.png` and ship `icon.dds` (or the other
/// way round), so the swapped extension is tried when the declared file is missing.
fn find_icon(root: Node, container: &ModContainer) -> Option<IconLocation> {
    let declared = text_of(child(root, "iconFilename")?);
    if declared.is_empty() {
        return None;
    }
    let swapped = match declared.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("png") => Some(format!("{stem}.dds")),
        Some((stem, ext)) if ext.eq_ignore_ascii_case("dds") => Some(format!("{stem}.png")),
        _ => None,
    };
    [Some(declared), swapped]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            let rel = container.find(&candidate)?;
            let kind = icon_kind(&rel)?;
            Some(IconLocation { rel, kind })
        })
}

fn icon_kind(rel: &str) -> Option<IconKind> {
    let (_, ext) = rel.rsplit_once('.')?;
    if ext.eq_ignore_ascii_case("dds") {
        Some(IconKind::Dds)
    } else if ext.eq_ignore_ascii_case("png") {
        Some(IconKind::Png)
    } else {
        None
    }
}

/// Escapes each `&` that does not start `name;`, `#digits;` or `#xhex;` as `&amp;`. CDATA
/// sections are left alone.
fn escape_bare_ampersands(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(at) = rest.find(['&', '<']) {
        let (before, tail) = rest.split_at(at);
        out.push_str(before);
        if let Some(cdata) = tail.strip_prefix("<![CDATA[") {
            let len = cdata.find("]]>").map_or(cdata.len(), |end| end + 3);
            let (inside, after) = cdata.split_at(len);
            out.push_str("<![CDATA[");
            out.push_str(inside);
            rest = after;
        } else if let Some(after) = tail.strip_prefix('<') {
            out.push('<');
            rest = after;
        } else if starts_with_reference(&tail[1..]) {
            out.push('&');
            rest = &tail[1..];
        } else {
            out.push_str("&amp;");
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Whether `after_amp` (the text after an `&`) begins an entity or character reference.
fn starts_with_reference(after_amp: &str) -> bool {
    let Some(end) = after_amp.find(';') else {
        return false;
    };
    let body = &after_amp[..end];
    if let Some(num) = body.strip_prefix("#x") {
        !num.is_empty() && num.chars().all(|c| c.is_ascii_hexdigit())
    } else if let Some(num) = body.strip_prefix('#') {
        !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
    } else {
        let mut chars = body.chars();
        chars
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':')
            && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_ampersands_are_escaped_and_references_kept() {
        assert_eq!(
            escape_bare_ampersands("Tom & Jerry &amp; &#65; &#x41; &lt;"),
            "Tom &amp; Jerry &amp; &#65; &#x41; &lt;"
        );
        assert_eq!(escape_bare_ampersands("a&b; c&"), "a&b; c&amp;");
        assert_eq!(
            escape_bare_ampersands("&;&#;&#x;&1;"),
            "&amp;;&amp;#;&amp;#x;&amp;1;"
        );
    }

    #[test]
    fn cdata_is_not_touched_by_the_escape() {
        assert_eq!(
            escape_bare_ampersands("<a><![CDATA[x & y]]> & z</a>"),
            "<a><![CDATA[x & y]]> &amp; z</a>"
        );
        // An unterminated CDATA section is copied through without panicking.
        assert_eq!(escape_bare_ampersands("<![CDATA[x & y"), "<![CDATA[x & y");
    }

    #[test]
    fn root_element_skips_bom_declaration_and_comments() {
        let xml = b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!-- c -->\n<placeable type=\"x\">";
        assert_eq!(root_element_name(xml).as_deref(), Some("placeable"));
        assert_eq!(root_element_name(b"<vehicle/>").as_deref(), Some("vehicle"));
        assert_eq!(root_element_name(b"<!-- never ends").as_deref(), None);
        assert_eq!(root_element_name(b"no xml").as_deref(), None);
        assert_eq!(root_element_name(b"").as_deref(), None);
    }

    #[test]
    fn base_game_paths_are_recognised() {
        assert!(is_base_game_path("$data/vehicles/x.xml"));
        assert!(is_base_game_path("$DATA/vehicles/x.xml"));
        assert!(!is_base_game_path("vehicles/x.xml"));
        assert!(!is_base_game_path("é"));
    }
}
