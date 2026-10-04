//! Lossless reader/writer for the YAML subset BL4 writes.
//!
//! The game uses block mappings and sequences only, plain or single-quoted
//! scalars, one custom tag (`!tags`), `key: ` with a trailing space for empty
//! and nested values, sequences at the same indent as their key and no final
//! newline. This module keeps every scalar's exact text and quoting, and the
//! key order, so `emit(parse(x)) == x` for every game-written save (checked
//! by the corpus test). New values follow the same rules.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    Single,
    Double,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scalar {
    /// unescaped value (for Double: the raw inner text)
    pub val: String,
    pub style: Style,
}

impl Scalar {
    pub fn plain(s: impl Into<String>) -> Scalar {
        Scalar { val: s.into(), style: Style::Plain }
    }
    /// A new scalar quoted the way the game would quote it.
    pub fn auto(s: impl Into<String>) -> Scalar {
        let val = s.into();
        let style = if needs_quotes(&val) { Style::Single } else { Style::Plain };
        Scalar { val, style }
    }
    pub fn is_null(&self) -> bool {
        self.style == Style::Plain && (self.val.is_empty() || self.val == "null" || self.val == "~")
    }
    fn emit(&self, out: &mut String) {
        match self.style {
            Style::Plain => out.push_str(&self.val),
            Style::Single => {
                out.push('\'');
                out.push_str(&self.val.replace('\'', "''"));
                out.push('\'');
            }
            Style::Double => {
                out.push('"');
                out.push_str(&self.val);
                out.push('"');
            }
        }
    }
}

/// Quote rule observed in game saves: serials (`@`), negative numbers,
/// strings with `,` `[` `:` `'`, and strings that would read as another type.
pub fn needs_quotes(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let first = s.chars().next().unwrap();
    if "@-!&*[]{}'\"%|>#?,`".contains(first) {
        // a plain negative number is still quoted by the game
        return true;
    }
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return true;
    }
    if s.contains(',') || s.contains('[') || s.contains(']') || s.contains('{') || s.contains('}') || s.contains('\'') {
        return true;
    }
    if s.starts_with(' ') || s.ends_with(' ') {
        return true;
    }
    false
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Scalar(Scalar),
    Map(Map),
    Seq(Seq),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map {
    pub tag: Option<String>,
    pub entries: Vec<(String, Node)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seq {
    pub tag: Option<String>,
    pub items: Vec<Node>,
}

#[derive(Debug, thiserror::Error)]
#[error("YAML parse error at line {line}: {msg}")]
pub struct ParseError {
    pub line: usize,
    pub msg: String,
}

impl Node {
    pub fn null() -> Node {
        Node::Scalar(Scalar::plain(""))
    }
    pub fn str(s: impl Into<String>) -> Node {
        Node::Scalar(Scalar::auto(s))
    }
    pub fn as_scalar(&self) -> Option<&Scalar> {
        match self {
            Node::Scalar(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        self.as_scalar().map(|s| s.val.as_str())
    }
    pub fn as_i64(&self) -> Option<i64> {
        self.as_str()?.trim().parse().ok()
    }
    pub fn as_f64(&self) -> Option<f64> {
        self.as_str()?.trim().parse().ok()
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self.as_str()?.to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Node::Map(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_map_mut(&mut self) -> Option<&mut Map> {
        match self {
            Node::Map(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_seq(&self) -> Option<&Seq> {
        match self {
            Node::Seq(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_seq_mut(&mut self) -> Option<&mut Seq> {
        match self {
            Node::Seq(s) => Some(s),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Node::Scalar(s) if s.is_null())
    }

    /// Navigate a dotted path (`state.currencies.cash`; `[n]` indexes sequences).
    pub fn get(&self, path: &str) -> Option<&Node> {
        let mut cur = self;
        for seg in split_path(path) {
            cur = match seg {
                Seg::Key(k) => cur.as_map()?.get(k)?,
                Seg::Idx(i) => cur.as_seq()?.items.get(i)?,
            };
        }
        Some(cur)
    }

    pub fn get_mut(&mut self, path: &str) -> Option<&mut Node> {
        let mut cur = self;
        for seg in split_path(path) {
            cur = match seg {
                Seg::Key(k) => cur.as_map_mut()?.get_mut(k)?,
                Seg::Idx(i) => cur.as_seq_mut()?.items.get_mut(i)?,
            };
        }
        Some(cur)
    }

    /// Navigate a path, creating missing maps along the way (a null scalar
    /// on the path is turned into an empty map). Returns the final node,
    /// created as null if missing.
    pub fn ensure(&mut self, path: &str) -> &mut Node {
        let mut cur = self;
        for seg in split_path(path) {
            match seg {
                Seg::Key(k) => {
                    if cur.is_null() {
                        *cur = Node::Map(Map::default());
                    }
                    let m = match cur {
                        Node::Map(m) => m,
                        _ => panic!("path {path}: not a map at {k}"),
                    };
                    if m.get(k).is_none() {
                        m.entries.push((k.to_string(), Node::null()));
                    }
                    cur = m.get_mut(k).unwrap();
                }
                Seg::Idx(i) => {
                    if cur.is_null() {
                        *cur = Node::Seq(Seq::default());
                    }
                    let s = match cur {
                        Node::Seq(s) => s,
                        _ => panic!("path {path}: not a sequence at [{i}]"),
                    };
                    while s.items.len() <= i {
                        s.items.push(Node::null());
                    }
                    cur = &mut s.items[i];
                }
            }
        }
        cur
    }

    /// Set a scalar, keeping the original quoting style when it still fits.
    pub fn set_scalar(&mut self, path: &str, val: impl Into<String>) {
        let val = val.into();
        let n = self.ensure(path);
        match n {
            Node::Scalar(s) if s.style == Style::Single || !needs_quotes(&val) => s.val = val,
            _ => *n = Node::str(val),
        }
    }

    pub fn emit(&self) -> String {
        let mut out = String::new();
        match self {
            Node::Map(m) => emit_map(m, 0, &mut out, None),
            Node::Seq(s) => emit_seq(s, 0, &mut out),
            Node::Scalar(s) => s.emit(&mut out),
        }
        if out.ends_with('\n') {
            out.pop();
        }
        out
    }
}

impl Map {
    pub fn get(&self, k: &str) -> Option<&Node> {
        self.entries.iter().find(|(kk, _)| kk == k).map(|(_, v)| v)
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut Node> {
        self.entries.iter_mut().find(|(kk, _)| kk == k).map(|(_, v)| v)
    }
    pub fn insert(&mut self, k: impl Into<String>, v: Node) {
        let k = k.into();
        match self.get_mut(&k) {
            Some(x) => *x = v,
            None => self.entries.push((k, v)),
        }
    }
    pub fn remove(&mut self, k: &str) -> Option<Node> {
        let i = self.entries.iter().position(|(kk, _)| kk == k)?;
        Some(self.entries.remove(i).1)
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

enum Seg<'a> {
    Key(&'a str),
    Idx(usize),
}

fn split_path(path: &str) -> Vec<Seg<'_>> {
    let mut out = vec![];
    for part in path.split('.').filter(|s| !s.is_empty()) {
        let mut rest = part;
        if let Some(b) = rest.find('[') {
            if b > 0 {
                out.push(Seg::Key(&rest[..b]));
            }
            rest = &rest[b..];
            while let Some(stripped) = rest.strip_prefix('[') {
                let e = stripped.find(']').unwrap_or(stripped.len());
                out.push(Seg::Idx(stripped[..e].parse().unwrap_or(0)));
                rest = stripped.get(e + 1..).unwrap_or("");
            }
        } else {
            out.push(Seg::Key(rest));
        }
    }
    out
}

// ---------------------------------------------------------------- emit

fn emit_tag(tag: &Option<String>, out: &mut String) {
    if let Some(t) = tag {
        out.push(' ');
        out.push_str(t);
    }
}

fn emit_map(m: &Map, ind: usize, out: &mut String, first_prefix: Option<&str>) {
    for (i, (k, v)) in m.entries.iter().enumerate() {
        match (i, first_prefix) {
            (0, Some(p)) => out.push_str(p),
            _ => push_indent(out, ind),
        }
        out.push_str(k);
        out.push(':');
        match v {
            Node::Scalar(s) => {
                out.push(' ');
                s.emit(out);
                out.push('\n');
            }
            Node::Map(c) => {
                if c.tag.is_some() {
                    emit_tag(&c.tag, out);
                } else {
                    out.push(' ');
                }
                out.push('\n');
                emit_map(c, ind + 2, out, None);
            }
            Node::Seq(c) => {
                if c.tag.is_some() {
                    emit_tag(&c.tag, out);
                } else {
                    out.push(' ');
                }
                out.push('\n');
                emit_seq(c, ind, out);
            }
        }
    }
}

fn emit_seq(s: &Seq, ind: usize, out: &mut String) {
    for item in &s.items {
        match item {
            Node::Scalar(sc) => {
                push_indent(out, ind);
                out.push_str("- ");
                sc.emit(out);
                out.push('\n');
            }
            Node::Map(m) => {
                let mut p = " ".repeat(ind);
                p.push_str("- ");
                if m.entries.is_empty() {
                    out.push_str(&p);
                    out.push('\n');
                } else {
                    emit_map(m, ind + 2, out, Some(&p));
                }
            }
            Node::Seq(c) => {
                push_indent(out, ind);
                out.push_str("- \n");
                emit_seq(c, ind + 2, out);
            }
        }
    }
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

// ---------------------------------------------------------------- parse

struct Line {
    ind: usize,
    text: String, // content after indentation
    no: usize,
}

pub fn parse(src: &str) -> Result<Node, ParseError> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines: Vec<Line> = Vec::new();
    for (no, raw) in src.split('\n').enumerate() {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            continue;
        }
        let ind = raw.len() - raw.trim_start_matches(' ').len();
        lines.push(Line { ind, text: raw[ind..].to_string(), no: no + 1 });
    }
    let mut p = Parser { lines, i: 0 };
    if p.lines.is_empty() {
        return Ok(Node::Map(Map::default()));
    }
    let ind = p.lines[0].ind;
    let n = p.block(ind)?;
    if p.i < p.lines.len() {
        return Err(p.err("unexpected content"));
    }
    Ok(n)
}

struct Parser {
    lines: Vec<Line>,
    i: usize,
}

fn is_seq_item(t: &str) -> bool {
    t == "-" || t.starts_with("- ")
}

impl Parser {
    fn err(&self, msg: &str) -> ParseError {
        let line = self.lines.get(self.i).map(|l| l.no).unwrap_or(0);
        ParseError { line, msg: msg.to_string() }
    }

    fn block(&mut self, ind: usize) -> Result<Node, ParseError> {
        if is_seq_item(&self.lines[self.i].text) {
            self.seq(ind, None)
        } else {
            self.map(ind, None)
        }
    }

    fn map(&mut self, ind: usize, tag: Option<String>) -> Result<Node, ParseError> {
        let mut m = Map { tag, entries: vec![] };
        while self.i < self.lines.len() {
            let l = &self.lines[self.i];
            if l.ind != ind || is_seq_item(&l.text) {
                if l.ind > ind {
                    return Err(self.err("bad indentation"));
                }
                break;
            }
            let text = l.text.clone();
            let (key, rest) = split_key(&text).ok_or_else(|| self.err("expected `key:`"))?;
            self.i += 1;
            let value = self.value_after_key(ind, rest)?;
            m.entries.push((key, value));
        }
        Ok(Node::Map(m))
    }

    /// Parse what follows `key:` (rest = text after the colon).
    fn value_after_key(&mut self, ind: usize, rest: &str) -> Result<Node, ParseError> {
        let r = rest.strip_prefix(' ').unwrap_or(rest);
        let mut tag = None;
        if r.starts_with('!') && !r.contains(' ') {
            tag = Some(r.to_string());
        } else if !r.is_empty() {
            return Ok(Node::Scalar(parse_scalar(r)));
        }
        // nested collection or null
        if self.i < self.lines.len() {
            let nl = &self.lines[self.i];
            if nl.ind == ind && is_seq_item(&nl.text) {
                return self.seq(ind, tag);
            }
            if nl.ind > ind {
                let ci = nl.ind;
                if is_seq_item(&nl.text) {
                    return self.seq(ci, tag);
                }
                return self.map(ci, tag);
            }
        }
        if let Some(t) = tag {
            // tagged but empty: keep it as an empty sequence with the tag
            return Ok(Node::Seq(Seq { tag: Some(t), items: vec![] }));
        }
        Ok(Node::null())
    }

    fn seq(&mut self, ind: usize, tag: Option<String>) -> Result<Node, ParseError> {
        let mut s = Seq { tag, items: vec![] };
        while self.i < self.lines.len() {
            let l = &self.lines[self.i];
            if l.ind != ind || !is_seq_item(&l.text) {
                break;
            }
            let rest = l.text.get(2..).unwrap_or("").to_string();
            if rest.is_empty() {
                self.i += 1;
                // nested block at deeper indent, or null
                if self.i < self.lines.len() && self.lines[self.i].ind > ind {
                    let ci = self.lines[self.i].ind;
                    s.items.push(self.block(ci)?);
                } else {
                    s.items.push(Node::Map(Map::default()));
                }
                continue;
            }
            if !rest.starts_with('\'') && !rest.starts_with('"') && split_key(&rest).is_some() {
                // mapping item: rewrite "- k: v" as "  k: v" at ind+2
                self.lines[self.i].ind = ind + 2;
                self.lines[self.i].text = rest;
                s.items.push(self.map(ind + 2, None)?);
            } else {
                self.i += 1;
                s.items.push(Node::Scalar(parse_scalar(&rest)));
            }
        }
        Ok(Node::Seq(s))
    }
}

/// Split `key: rest` / `key:` into (key, rest-after-colon).
fn split_key(t: &str) -> Option<(String, &str)> {
    if let Some(stripped) = t.strip_prefix('\'') {
        // quoted key
        let mut i = 0;
        let b = stripped.as_bytes();
        let mut key = String::new();
        while i < b.len() {
            if b[i] == b'\'' {
                if i + 1 < b.len() && b[i + 1] == b'\'' {
                    key.push('\'');
                    i += 2;
                    continue;
                }
                let after = &stripped[i + 1..];
                let after = after.strip_prefix(':')?;
                return Some((format!("'{}'", key.replace('\'', "''")), after));
            }
            key.push(b[i] as char);
            i += 1;
        }
        return None;
    }
    if let Some(p) = t.find(": ") {
        return Some((t[..p].to_string(), &t[p + 1..]));
    }
    if let Some(k) = t.strip_suffix(':') {
        if !k.contains(' ') || !k.is_empty() {
            return Some((k.to_string(), ""));
        }
    }
    None
}

fn parse_scalar(r: &str) -> Scalar {
    if let Some(inner) = r.strip_prefix('\'') {
        if let Some(inner) = inner.strip_suffix('\'') {
            return Scalar { val: inner.replace("''", "'"), style: Style::Single };
        }
    }
    if let Some(inner) = r.strip_prefix('"') {
        if let Some(inner) = inner.strip_suffix('"') {
            return Scalar { val: inner.to_string(), style: Style::Double };
        }
    }
    Scalar { val: r.to_string(), style: Style::Plain }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.emit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_roundtrip() {
        let src = "state: \n  char_name: Vex\n  experience: \n  - type: Character\n    level: 48\n  - type: Specialization\n    level: 2\n  inventory: \n    items: \n      backpack: \n        slot_0: \n          serial: '@Ugr$TAm/&nF'\n          state_flags: 1\n  empty: \n  tags: !tags\n  - a\n  - b\nglobals: \n  x: TRUE";
        let n = parse(src).unwrap();
        assert_eq!(n.emit(), src);
        assert_eq!(n.get("state.experience[0].level").unwrap().as_i64(), Some(48));
        assert_eq!(n.get("state.inventory.items.backpack.slot_0.serial").unwrap().as_str(), Some("@Ugr$TAm/&nF"));
    }

    #[test]
    fn set_and_insert() {
        let mut n = parse("a: \n  b: 1").unwrap();
        n.set_scalar("a.b", "2");
        n.set_scalar("a.c.d", "@x");
        assert_eq!(n.emit(), "a: \n  b: 2\n  c: \n    d: '@x'");
    }
}
