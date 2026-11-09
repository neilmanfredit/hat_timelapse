//! Reading and updating XMP sidecars.
//!
//! Lightroom and LightCraft both store develop settings as Camera Raw (`crs:`)
//! properties on the `rdf:Description` of an XMP packet. Hat Timelapse only touches
//! the properties it is asked to change; everything else in the sidecar
//! (keywords, ratings, masks, history) is passed through untouched.

use anyhow::{bail, Context, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::BTreeMap;
use std::path::Path;

pub const NS_CRS: &str = "http://ns.adobe.com/camera-raw-settings/1.0/";
pub const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";
/// LightCraft's own namespace; `lc:settings` holds its complete develop settings as JSON.
pub const NS_LC: &str = "http://ns.lightcraft.app/lc/1.0/";

const KNOWN_NS: &[(&str, &str)] = &[("crs", NS_CRS), ("xmp", NS_XMP), ("lc", NS_LC)];

pub const EMPTY_SIDECAR: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Hat Timelapse">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/">
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

/// A set of changes to apply to a sidecar. Keys are qualified names such as
/// `crs:Exposure2012` or `xmp:Label`.
#[derive(Debug, Default, Clone)]
pub struct Edit {
    pub set: BTreeMap<String, String>,
    /// Only written if the sidecar does not already carry the property.
    pub set_if_absent: BTreeMap<String, String>,
    pub remove: Vec<String>,
    /// Structured properties (tone curves, masks, looks) as raw XML, keyed by
    /// qualified name. Each replaces any existing property of that name.
    pub elements: BTreeMap<String, String>,
    /// Namespace declarations (prefix → URI) the copied properties need.
    pub namespaces: BTreeMap<String, String>,
}

impl Edit {
    pub fn set(mut self, k: &str, v: impl Into<String>) -> Self {
        self.set.insert(k.to_string(), v.into());
        self
    }
    pub fn set_if_absent(mut self, k: &str, v: impl Into<String>) -> Self {
        self.set_if_absent.insert(k.to_string(), v.into());
        self
    }
    pub fn remove(mut self, k: &str) -> Self {
        self.remove.push(k.to_string());
        self
    }
}

/// All simple properties (attributes and text-only child elements) of every
/// `rdf:Description`, keyed by qualified name.
pub fn read_properties(xml: &str) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut out = BTreeMap::new();
    let mut desc_depth: Option<usize> = None;
    let mut depth = 0usize;
    let mut current: Option<String> = None;
    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                depth += 1;
                let name = qname(&e);
                if name == "rdf:Description" {
                    collect_attrs(&e, &mut out)?;
                    desc_depth = Some(depth);
                } else if desc_depth == Some(depth - 1) {
                    current = Some(name);
                } else {
                    current = None;
                }
            }
            Event::Empty(e) => {
                if qname(&e) == "rdf:Description" {
                    collect_attrs(&e, &mut out)?;
                }
            }
            Event::Text(t) => {
                if let Some(k) = &current {
                    let v = t.unescape()?.trim().to_string();
                    if !v.is_empty() {
                        out.insert(k.clone(), v);
                    }
                }
            }
            Event::End(_) => {
                if desc_depth == Some(depth) {
                    desc_depth = None;
                }
                current = None;
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn qname(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.name().as_ref()).into_owned()
}

fn collect_attrs(e: &BytesStart, out: &mut BTreeMap<String, String>) -> Result<()> {
    for a in e.attributes() {
        let a = a?;
        let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        if k.starts_with("xmlns") || k == "rdf:about" {
            continue;
        }
        out.insert(k, a.unescape_value()?.into_owned());
    }
    Ok(())
}

/// Everything a sidecar's develop settings consist of: simple properties,
/// structured properties as raw XML, and the namespaces they use.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Snapshot {
    pub properties: BTreeMap<String, String>,
    /// Child elements of `rdf:Description` that contain further elements.
    pub elements: BTreeMap<String, String>,
    pub namespaces: BTreeMap<String, String>,
}

pub fn snapshot(xml: &str) -> Result<Snapshot> {
    let properties = read_properties(xml)?;
    let mut elements = BTreeMap::new();
    let mut namespaces = BTreeMap::new();
    let mut reader = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut desc_depth: Option<usize> = None;
    // (name, start offset, has nested element)
    let mut open: Option<(String, usize, bool)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader.read_event()?;
        let mut note_ns = |e: &BytesStart| -> Result<()> {
            for a in e.attributes() {
                let a = a?;
                let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                if let Some(p) = k.strip_prefix("xmlns:") {
                    namespaces
                        .entry(p.to_string())
                        .or_insert(a.unescape_value()?.into_owned());
                }
            }
            Ok(())
        };
        match ev {
            Event::Start(e) => {
                note_ns(&e)?;
                depth += 1;
                let name = qname(&e);
                if desc_depth.is_none() && name == "rdf:Description" {
                    desc_depth = Some(depth);
                } else if desc_depth == Some(depth - 1) {
                    open = Some((name, before, false));
                } else if let Some(o) = open.as_mut() {
                    o.2 = true;
                }
            }
            Event::Empty(e) => {
                note_ns(&e)?;
                if let Some(o) = open.as_mut() {
                    o.2 = true;
                }
            }
            Event::End(_) => {
                if desc_depth == Some(depth - 1) {
                    if let Some((name, start, nested)) = open.take() {
                        if nested {
                            let end = reader.buffer_position() as usize;
                            elements.insert(name, xml[start..end].to_string());
                        }
                    }
                } else if desc_depth == Some(depth) {
                    desc_depth = Some(usize::MAX); // only the first Description
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(Snapshot {
        properties,
        elements,
        namespaces,
    })
}

pub fn snapshot_file(path: &Path) -> Result<Snapshot> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    snapshot(&xml).with_context(|| format!("parsing {}", path.display()))
}

/// Apply `edit` to the first `rdf:Description` in `xml`. Properties are written
/// as attributes (Lightroom's own layout); any element form of a changed
/// property is removed so there is only one value.
pub fn apply(xml: &str, edit: &Edit) -> Result<String> {
    let existing = read_properties(xml)?;
    let mut set = edit.set.clone();
    for (k, v) in &edit.set_if_absent {
        if !existing.contains_key(k) && !set.contains_key(k) {
            set.insert(k.clone(), v.clone());
        }
    }
    let mut remove = edit.remove.clone();
    remove.extend(edit.elements.keys().cloned());
    let touched = |k: &str| set.contains_key(k) || remove.iter().any(|r| r == k);
    let write_elements = |w: &mut Writer<Vec<u8>>| {
        for raw in edit.elements.values() {
            w.get_mut().extend_from_slice(b"\n   ");
            w.get_mut().extend_from_slice(raw.as_bytes());
        }
        if !edit.elements.is_empty() {
            w.get_mut().extend_from_slice(b"\n  ");
        }
    };

    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Vec::with_capacity(xml.len() + 1024));
    let mut done = false;
    let mut depth = 0usize;
    let mut desc_depth: Option<usize> = None;
    let mut skip_until: Option<usize> = None;

    loop {
        let ev = reader.read_event()?;
        if let Some(d) = skip_until {
            match &ev {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth < d {
                        skip_until = None;
                    }
                }
                Event::Eof => bail!("unexpected end of XMP"),
                _ => {}
            }
            continue;
        }
        match ev {
            Event::Start(e) => {
                depth += 1;
                let name = qname(&e);
                if !done && name == "rdf:Description" {
                    writer.write_event(Event::Start(rewrite(
                        &e,
                        &set,
                        &remove,
                        &edit.namespaces,
                    )?))?;
                    desc_depth = Some(depth);
                    done = true;
                } else if desc_depth == Some(depth - 1) && touched(&name) {
                    skip_until = Some(depth);
                } else {
                    writer.write_event(Event::Start(e))?;
                }
            }
            Event::Empty(e) => {
                let name = qname(&e);
                if !done && name == "rdf:Description" {
                    let start = rewrite(&e, &set, &remove, &edit.namespaces)?;
                    if edit.elements.is_empty() {
                        writer.write_event(Event::Empty(start))?;
                    } else {
                        writer.write_event(Event::Start(start))?;
                        write_elements(&mut writer);
                        writer.write_event(Event::End(BytesEnd::new("rdf:Description")))?;
                    }
                    done = true;
                } else if desc_depth == Some(depth) && touched(&name) {
                    // drop
                } else {
                    writer.write_event(Event::Empty(e))?;
                }
            }
            Event::End(e) => {
                if desc_depth == Some(depth) {
                    write_elements(&mut writer);
                    desc_depth = None;
                }
                depth = depth.saturating_sub(1);
                writer.write_event(Event::End(e))?;
            }
            Event::Eof => break,
            other => writer.write_event(other)?,
        }
    }
    if !done {
        bail!("XMP has no rdf:Description");
    }
    Ok(String::from_utf8(writer.into_inner())?)
}

fn rewrite<'a>(
    e: &BytesStart<'a>,
    set: &BTreeMap<String, String>,
    remove: &[String],
    namespaces: &BTreeMap<String, String>,
) -> Result<BytesStart<'static>> {
    let mut out = BytesStart::new(qname(e));
    let mut declared: Vec<String> = Vec::new();
    for a in e.attributes() {
        let a = a?;
        let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        if let Some(p) = k.strip_prefix("xmlns:") {
            declared.push(p.to_string());
        }
        if set.contains_key(&k) || remove.contains(&k) {
            continue;
        }
        out.push_attribute((k.as_str(), a.unescape_value()?.as_ref()));
    }
    // Declare what the new properties need. `rdf` and `x` are declared by the
    // enclosing elements of every XMP packet.
    let needed = set
        .keys()
        .filter_map(|k| k.split_once(':').map(|(p, _)| p))
        .chain(
            namespaces
                .keys()
                .map(String::as_str)
                .filter(|p| !matches!(*p, "rdf" | "x" | "xml")),
        );
    for prefix in needed.collect::<Vec<_>>() {
        if !declared.iter().any(|d| d == prefix) {
            let uri = namespaces
                .get(prefix)
                .map(String::as_str)
                .or_else(|| KNOWN_NS.iter().find(|(p, _)| *p == prefix).map(|(_, u)| *u))
                .with_context(|| format!("unknown XMP namespace prefix `{prefix}`"))?;
            out.push_attribute((format!("xmlns:{prefix}").as_str(), uri));
            declared.push(prefix.to_string());
        }
    }
    for (k, v) in set {
        out.push_attribute((k.as_str(), v.as_str()));
    }
    Ok(out)
}

pub fn read_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    read_properties(&xml).with_context(|| format!("parsing {}", path.display()))
}

/// Update (or create) the sidecar at `path`. Writes via a temporary file so a
/// crash never leaves a half-written sidecar for Lightroom to choke on.
pub fn update_file(path: &Path, edit: &Edit) -> Result<()> {
    let xml = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => EMPTY_SIDECAR.to_string(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let new = apply(&xml, edit).with_context(|| format!("updating {}", path.display()))?;
    let tmp = path.with_extension("xmp.tmp");
    std::fs::write(&tmp, new)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LR: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmp:Rating="3"
    crs:Exposure2012="+0.50"
    crs:Contrast2012="+10">
   <crs:Shadows2012>+25</crs:Shadows2012>
   <crs:ToneCurvePV2012>
    <rdf:Seq><rdf:li>0, 0</rdf:li><rdf:li>255, 255</rdf:li></rdf:Seq>
   </crs:ToneCurvePV2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn reads_attributes_and_elements() {
        let p = read_properties(LR).unwrap();
        assert_eq!(p["crs:Exposure2012"], "+0.50");
        assert_eq!(p["crs:Shadows2012"], "+25");
        assert_eq!(p["xmp:Rating"], "3");
        assert!(!p.contains_key("crs:ToneCurvePV2012"));
    }

    #[test]
    fn updates_and_preserves() {
        let edit = Edit::default()
            .set("crs:Exposure2012", "-1.25")
            .set("crs:Shadows2012", "+40")
            .set_if_absent("crs:Contrast2012", "+99")
            .set_if_absent("crs:ProcessVersion", "11.0");
        let out = apply(LR, &edit).unwrap();
        let p = read_properties(&out).unwrap();
        assert_eq!(p["crs:Exposure2012"], "-1.25");
        assert_eq!(p["crs:Shadows2012"], "+40");
        assert_eq!(p["crs:Contrast2012"], "+10");
        assert_eq!(p["crs:ProcessVersion"], "11.0");
        assert_eq!(p["xmp:Rating"], "3");
        assert!(out.contains("<rdf:li>255, 255</rdf:li>"));
        assert_eq!(out.matches("Shadows2012").count(), 1);
    }

    #[test]
    fn copies_structured_settings_between_sidecars() {
        let snap = snapshot(LR).unwrap();
        assert!(snap.elements.contains_key("crs:ToneCurvePV2012"));
        assert!(!snap.elements.contains_key("crs:Shadows2012"));
        assert_eq!(snap.namespaces["crs"], NS_CRS);

        let edit = Edit {
            elements: snap.elements.clone(),
            namespaces: snap.namespaces.clone(),
            ..Default::default()
        }
        .set("crs:Exposure2012", "+0.25");
        // Into a fresh, self-closing sidecar.
        let out = apply(
            EMPTY_SIDECAR
                .replace(">\n  </rdf:Description>", "/>")
                .as_str(),
            &edit,
        )
        .unwrap();
        assert!(out.contains("<rdf:li>255, 255</rdf:li>"), "{out}");
        assert_eq!(snapshot(&out).unwrap().elements, snap.elements);
        assert_eq!(read_properties(&out).unwrap()["crs:Exposure2012"], "+0.25");

        // Applying again replaces rather than duplicates.
        let twice = apply(&out, &edit).unwrap();
        assert_eq!(twice.matches("ToneCurvePV2012>").count(), 2);
    }

    #[test]
    fn declares_missing_namespace() {
        let edit = Edit::default().set("xmp:Label", "Keyframe");
        let out = apply(EMPTY_SIDECAR, &edit).unwrap();
        assert!(out.contains(r#"xmlns:xmp="http://ns.adobe.com/xap/1.0/""#));
        assert_eq!(read_properties(&out).unwrap()["xmp:Label"], "Keyframe");
    }
}
