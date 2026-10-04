//! Lossless codec for BL4 item serials (`@Ug...`).
//!
//! Pipeline: strip `@U`, custom base85 -> bytes, then a bitstream read
//! LSB-first within each byte (equivalent to "mirror each byte, read MSB-first").
//! After a 7-bit magic (0b0010000) the stream is a list of tokens:
//!
//! | prefix | token                                    |
//! |--------|------------------------------------------|
//! | `00`   | separator                                |
//! | `01`   | soft separator                           |
//! | `100`  | varint (4-bit LSB-first nibbles + cont)  |
//! | `110`  | varbit (5-bit length + value)            |
//! | `101`  | part: varint index + value block         |
//! | `111`  | string: varint length + 7-bit chars      |
//!
//! Every token remembers exactly how it was written, so `encode(decode(s)) == s`
//! for every serial the game produces; this is checked by the self-test over
//! all test saves.

use std::fmt;

const ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SerialError {
    #[error("serial must start with @Ug")]
    Prefix,
    #[error("invalid base85 character {0:?}")]
    BadChar(char),
    #[error("bad magic header")]
    Magic,
    #[error("serial too short")]
    Short,
}

/// How a part token's value block was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartVal {
    /// `0 10`: no value. The part indexes the item's own category.
    None,
    /// `1 <varint> 000`: one value. The index is a category, the value a part in it.
    Single(u64),
    /// `0 01 [01] (100 v | 110 v)* [00]`: several parts of category `index`.
    List(PartList),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartList {
    /// Values with their encoding (false = varint, true = varbit).
    pub vals: Vec<(u64, bool)>,
    /// The game writes a soft separator (`01`) right after the list marker.
    pub lead_soft: bool,
    /// Whether the list was closed by an explicit `00`.
    pub terminated: bool,
}

impl PartList {
    pub fn new(vals: &[u64]) -> Self {
        PartList { vals: vals.iter().map(|&v| (v, false)).collect(), lead_soft: true, terminated: true }
    }
    pub fn values(&self) -> impl Iterator<Item = u64> + '_ {
        self.vals.iter().map(|v| v.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Sep,
    Soft,
    Int(u64),
    /// varbit value and the bit width it was written with (canonical = minimal)
    Bit(u64, u8),
    Part { idx: u64, val: PartVal },
    Str(String),
}

impl Tok {
    pub fn var(&self) -> Option<u64> {
        match self {
            Tok::Int(v) | Tok::Bit(v, _) => Some(*v),
            _ => None,
        }
    }
    pub fn bit(v: u64) -> Tok {
        Tok::Bit(v, min_width(v))
    }
}

fn min_width(v: u64) -> u8 {
    (64 - v.leading_zeros()) as u8
}

// ---------------------------------------------------------------- base85

pub fn b85_decode(s: &str) -> Result<Vec<u8>, SerialError> {
    let mut lut = [255u8; 256];
    for (i, &c) in ALPHABET.iter().enumerate() {
        lut[c as usize] = i as u8;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() * 4 / 5 + 4);
    for chunk in b.chunks(5) {
        let mut v: u64 = 0;
        for &c in chunk {
            let d = lut[c as usize];
            if d == 255 {
                return Err(SerialError::BadChar(c as char));
            }
            v = v * 85 + d as u64;
        }
        for _ in chunk.len()..5 {
            v = v * 85 + 84;
        }
        let n = if chunk.len() == 5 { 4 } else { chunk.len().saturating_sub(1) };
        let bytes = (v as u32).to_be_bytes();
        out.extend_from_slice(&bytes[..n]);
    }
    Ok(out)
}

pub fn b85_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 5 / 4 + 5);
    for chunk in bytes.chunks(4) {
        let mut buf = [0u8; 4];
        buf[..chunk.len()].copy_from_slice(chunk);
        let mut v = u32::from_be_bytes(buf) as u64;
        let mut chars = [0u8; 5];
        for i in (0..5).rev() {
            chars[i] = ALPHABET[(v % 85) as usize];
            v /= 85;
        }
        let n = if chunk.len() == 4 { 5 } else { chunk.len() + 1 };
        for &c in &chars[..n] {
            out.push(c as char);
        }
    }
    out
}

// ---------------------------------------------------------------- bits

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn left(&self) -> usize {
        self.d.len() * 8 - self.p.min(self.d.len() * 8)
    }
    fn bit(&mut self) -> Option<u64> {
        if self.p >= self.d.len() * 8 {
            return None;
        }
        let b = (self.d[self.p >> 3] >> (self.p & 7)) & 1;
        self.p += 1;
        Some(b as u64)
    }
    /// k bits in stream order, first bit = most significant (prefixes).
    fn bits(&mut self, k: usize) -> Option<u64> {
        let mut v = 0;
        for _ in 0..k {
            v = (v << 1) | self.bit()?;
        }
        Some(v)
    }
    /// k bits as an LSB-first value (data fields).
    fn lsb(&mut self, k: usize) -> Option<u64> {
        let mut v = 0;
        for i in 0..k {
            v |= self.bit()? << i;
        }
        Some(v)
    }
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0;
        for i in 0..4 {
            v |= self.lsb(4)? << (4 * i);
            if self.bit()? == 0 {
                return Some(v);
            }
        }
        Some(v)
    }
    fn varbit(&mut self) -> Option<(u64, u8)> {
        let n = self.lsb(5)? as usize;
        Some((if n == 0 { 0 } else { self.lsb(n)? }, n as u8))
    }
}

#[derive(Default)]
struct Writer {
    d: Vec<u8>,
    p: usize,
}

impl Writer {
    fn bit(&mut self, b: u64) {
        if self.p >> 3 >= self.d.len() {
            self.d.push(0);
        }
        if b & 1 == 1 {
            self.d[self.p >> 3] |= 1 << (self.p & 7);
        }
        self.p += 1;
    }
    fn bits(&mut self, v: u64, k: usize) {
        for i in (0..k).rev() {
            self.bit(v >> i);
        }
    }
    fn lsb(&mut self, v: u64, k: usize) {
        for i in 0..k {
            self.bit(v >> i);
        }
    }
    fn varint(&mut self, mut v: u64) {
        loop {
            self.lsb(v & 0xF, 4);
            v >>= 4;
            if v == 0 {
                self.bit(0);
                return;
            }
            self.bit(1);
        }
    }
    fn varbit(&mut self, v: u64, width: u8) {
        let w = width.max(min_width(v)) as usize;
        self.lsb(w as u64, 5);
        self.lsb(v, w);
    }
}

// ---------------------------------------------------------------- tokens

pub fn decode_tokens(serial: &str) -> Result<(Vec<Tok>, usize), SerialError> {
    let body = serial.strip_prefix("@U").ok_or(SerialError::Prefix)?;
    if !body.starts_with('g') {
        return Err(SerialError::Prefix);
    }
    let bytes = b85_decode(body)?;
    if bytes.len() < 2 {
        return Err(SerialError::Short);
    }
    let mut r = Reader { d: &bytes, p: 0 };
    if r.bits(7) != Some(0b0010000) {
        return Err(SerialError::Magic);
    }
    let mut toks = Vec::new();
    while r.left() >= 2 {
        let Some(p2) = r.bits(2) else { break };
        match p2 {
            0b00 => {
                toks.push(Tok::Sep);
                if r.left() < 8 {
                    break;
                }
            }
            0b01 => toks.push(Tok::Soft),
            _ => {
                let Some(b3) = r.bit() else { break };
                let tok = match (p2 << 1) | b3 {
                    0b100 => r.varint().map(Tok::Int),
                    0b110 => r.varbit().map(|(v, w)| Tok::Bit(v, w)),
                    0b101 => read_part(&mut r),
                    _ => read_str(&mut r),
                };
                match tok {
                    Some(t) => toks.push(t),
                    // trailing padding that does not form a token ends the stream
                    None => break,
                }
            }
        }
    }
    Ok((toks, bytes.len()))
}

fn read_part(r: &mut Reader) -> Option<Tok> {
    let idx = r.varint()?;
    if r.bit()? == 1 {
        let v = r.varint()?;
        r.bits(3)?;
        return Some(Tok::Part { idx, val: PartVal::Single(v) });
    }
    match r.bits(2)? {
        0b10 => Some(Tok::Part { idx, val: PartVal::None }),
        0b01 => {
            let mut l = PartList { vals: vec![], lead_soft: false, terminated: false };
            let save = r.p;
            if r.bits(2) == Some(0b01) {
                l.lead_soft = true;
            } else {
                r.p = save;
            }
            loop {
                let save = r.p;
                let Some(pk) = r.bits(2) else { break };
                if pk == 0b00 {
                    l.terminated = true;
                    break;
                }
                let Some(b3) = r.bit() else {
                    r.p = save;
                    break;
                };
                match (pk << 1) | b3 {
                    0b100 => l.vals.push((r.varint()?, false)),
                    0b110 => l.vals.push((r.varbit()?.0, true)),
                    _ => {
                        r.p = save;
                        break;
                    }
                }
            }
            Some(Tok::Part { idx, val: PartVal::List(l) })
        }
        _ => None,
    }
}

fn read_str(r: &mut Reader) -> Option<Tok> {
    let n = r.varint()? as usize;
    if n > 256 {
        return None;
    }
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        s.push(r.lsb(7)? as u8 as char);
    }
    Some(Tok::Str(s))
}

pub fn encode_tokens(toks: &[Tok], min_len: usize) -> String {
    let mut w = Writer::default();
    w.bits(0b0010000, 7);
    for t in toks {
        match t {
            Tok::Sep => w.bits(0b00, 2),
            Tok::Soft => w.bits(0b01, 2),
            Tok::Int(v) => {
                w.bits(0b100, 3);
                w.varint(*v);
            }
            Tok::Bit(v, wd) => {
                w.bits(0b110, 3);
                w.varbit(*v, *wd);
            }
            Tok::Part { idx, val } => {
                w.bits(0b101, 3);
                w.varint(*idx);
                match val {
                    PartVal::None => {
                        w.bit(0);
                        w.bits(0b10, 2);
                    }
                    PartVal::Single(v) => {
                        w.bit(1);
                        w.varint(*v);
                        w.bits(0, 3);
                    }
                    PartVal::List(l) => {
                        w.bit(0);
                        w.bits(0b01, 2);
                        if l.lead_soft {
                            w.bits(0b01, 2);
                        }
                        for &(v, is_bit) in &l.vals {
                            if is_bit {
                                w.bits(0b110, 3);
                                w.varbit(v, min_width(v));
                            } else {
                                w.bits(0b100, 3);
                                w.varint(v);
                            }
                        }
                        if l.terminated {
                            w.bits(0b00, 2);
                        }
                    }
                }
            }
            Tok::Str(s) => {
                w.bits(0b111, 3);
                w.varint(s.len() as u64);
                for c in s.bytes() {
                    w.lsb(c as u64 & 0x7F, 7);
                }
            }
        }
    }
    let mut bytes = w.d;
    while bytes.len() < min_len {
        bytes.push(0);
    }
    format!("@U{}", b85_encode(&bytes))
}

// ---------------------------------------------------------------- item view

/// One part reference: (category, part index). For a plain `{n}` token the
/// category is the item's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartRef {
    pub cat: u32,
    pub idx: u32,
}

/// A decoded item serial. Holds the exact token stream; semantic accessors
/// read from it and edits rewrite it.
#[derive(Debug, Clone, PartialEq)]
pub struct Serial {
    pub toks: Vec<Tok>,
    /// decoded byte length, kept so unchanged serials re-encode identically
    pub byte_len: usize,
}

impl Serial {
    pub fn decode(s: &str) -> Result<Serial, SerialError> {
        let s = s.trim();
        let (toks, byte_len) = decode_tokens(s)?;
        if toks.is_empty() || toks[0].var().is_none() {
            return Err(SerialError::Short);
        }
        Ok(Serial { toks, byte_len })
    }

    pub fn encode(&self) -> String {
        encode_tokens(&self.toks, self.byte_len)
    }

    /// Category (serial index of the item type). VarInt for weapons, VarBit for gear.
    pub fn category(&self) -> u32 {
        self.toks[0].var().unwrap_or(0) as u32
    }

    pub fn is_varbit_first(&self) -> bool {
        matches!(self.toks[0], Tok::Bit(..))
    }

    /// Index of the first token of the parts section (after the first `| |`).
    pub fn parts_start(&self) -> Option<usize> {
        (0..self.toks.len().saturating_sub(1))
            .find(|&i| self.toks[i] == Tok::Sep && self.toks[i + 1] == Tok::Sep)
            .map(|i| i + 2)
    }

    /// End (exclusive) of the parts section: next separator after parts_start.
    pub fn parts_end(&self) -> usize {
        let s = self.parts_start().unwrap_or(self.toks.len());
        (s..self.toks.len()).find(|&i| self.toks[i] == Tok::Sep).unwrap_or(self.toks.len())
    }

    /// Header groups: tokens before the parts section split by `Sep`, each group
    /// a list of var values.
    pub fn header_groups(&self) -> Vec<Vec<(usize, u64)>> {
        let end = self.parts_start().map(|s| s - 2).unwrap_or(self.toks.len());
        let mut groups = vec![vec![]];
        for (i, t) in self.toks[..end].iter().enumerate() {
            match t {
                Tok::Sep => groups.push(vec![]),
                _ => {
                    if let Some(v) = t.var() {
                        groups.last_mut().unwrap().push((i, v));
                    }
                }
            }
        }
        groups
    }

    /// Token index holding the level, when the header has the usual
    /// `cat, 0, 1, level` shape.
    fn level_tok(&self) -> Option<usize> {
        let g = self.header_groups();
        let g0 = g.first()?;
        if g0.len() == 4 && g0[1].1 == 0 && g0[2].1 == 1 {
            Some(g0[3].0)
        } else {
            None
        }
    }

    pub fn level(&self) -> Option<u32> {
        self.level_tok().and_then(|i| self.toks[i].var()).map(|v| v as u32)
    }

    pub fn set_level(&mut self, level: u32) -> bool {
        match self.level_tok() {
            Some(i) => {
                self.toks[i] = match self.toks[i] {
                    Tok::Bit(..) => Tok::bit(level as u64),
                    _ => Tok::Int(level as u64),
                };
                true
            }
            None => false,
        }
    }

    /// Random seed: the value after the `2` marker in a header group.
    pub fn seed(&self) -> Option<u64> {
        for g in self.header_groups().iter().skip(1) {
            if g.len() == 2 && g[0].1 == 2 {
                return Some(g[1].1);
            }
        }
        None
    }

    pub fn set_seed(&mut self, seed: u64) -> bool {
        for g in self.header_groups().iter().skip(1) {
            if g.len() == 2 && g[0].1 == 2 {
                let i = g[1].0;
                self.toks[i] = match self.toks[i] {
                    Tok::Bit(..) => Tok::bit(seed),
                    _ => Tok::Int(seed),
                };
                return true;
            }
        }
        false
    }

    /// All part references in the parts section, in order, with the token
    /// index they come from.
    pub fn part_refs(&self) -> Vec<(usize, PartRef)> {
        let cat = self.category();
        let Some(s) = self.parts_start() else { return vec![] };
        let e = self.parts_end();
        let mut out = vec![];
        for i in s..e {
            if let Tok::Part { idx, val } = &self.toks[i] {
                match val {
                    PartVal::None => out.push((i, PartRef { cat, idx: *idx as u32 })),
                    PartVal::Single(v) => out.push((i, PartRef { cat: *idx as u32, idx: *v as u32 })),
                    PartVal::List(l) => {
                        for v in l.values() {
                            out.push((i, PartRef { cat: *idx as u32, idx: v as u32 }));
                        }
                    }
                }
            }
        }
        out
    }

    pub fn parts(&self) -> Vec<PartRef> {
        self.part_refs().into_iter().map(|x| x.1).collect()
    }

    /// Tokens after the parts section (string tags such as cosmetics, trailing data).
    pub fn trailer(&self) -> &[Tok] {
        &self.toks[self.parts_end()..]
    }

    fn ensure_parts_section(&mut self) -> usize {
        if let Some(s) = self.parts_start() {
            return s;
        }
        self.toks.push(Tok::Sep);
        self.toks.push(Tok::Sep);
        self.toks.len()
    }

    fn touched(&mut self) {
        // an edited payload may shrink; let the length follow the tokens
        self.byte_len = 0;
    }

    /// Replace the n-th part reference (in `part_refs()` order). The token
    /// layout of untouched parts is preserved; a value inside a list stays in
    /// that list when the category is unchanged.
    pub fn replace_part(&mut self, n: usize, new: PartRef) -> bool {
        let refs = self.part_refs();
        let Some(&(ti, old)) = refs.get(n) else { return false };
        let own = self.category();
        // position inside a list token
        let k = refs[..n].iter().filter(|(t, _)| *t == ti).count();
        if old.cat == new.cat {
            match &mut self.toks[ti] {
                Tok::Part { idx, val: PartVal::None } => *idx = new.idx as u64,
                Tok::Part { val: PartVal::Single(v), .. } => *v = new.idx as u64,
                Tok::Part { val: PartVal::List(l), .. } => l.vals[k].0 = new.idx as u64,
                _ => return false,
            }
        } else {
            let tok = part_token(own, new);
            let in_list = matches!(&self.toks[ti], Tok::Part { val: PartVal::List(l), .. } if l.vals.len() > 1);
            if in_list {
                if let Tok::Part { val: PartVal::List(l), .. } = &mut self.toks[ti] {
                    l.vals.remove(k);
                }
                self.toks.insert(ti + 1, tok);
            } else {
                self.toks[ti] = tok;
            }
        }
        self.touched();
        true
    }

    /// Remove the n-th part reference.
    pub fn remove_part(&mut self, n: usize) -> bool {
        let refs = self.part_refs();
        let Some(&(ti, _)) = refs.get(n) else { return false };
        let k = refs[..n].iter().filter(|(t, _)| *t == ti).count();
        let mut drop_tok = true;
        if let Tok::Part { val: PartVal::List(l), .. } = &mut self.toks[ti] {
            if l.vals.len() > 1 {
                l.vals.remove(k);
                drop_tok = false;
            }
        }
        if drop_tok {
            self.toks.remove(ti);
        }
        self.touched();
        true
    }

    /// Add a part. Own-category parts become `{n}`; a foreign part joins an
    /// existing list of its category when the item already has one, else it
    /// becomes `{cat:n}`. New tokens go at the end of the parts section.
    pub fn add_part(&mut self, p: PartRef) {
        let own = self.category();
        let s = self.ensure_parts_section();
        let e = self.parts_end();
        if p.cat != own {
            for t in &mut self.toks[s..e] {
                if let Tok::Part { idx, val: PartVal::List(l) } = t {
                    if *idx as u32 == p.cat {
                        l.vals.push((p.idx as u64, false));
                        self.touched();
                        return;
                    }
                }
            }
        }
        self.toks.insert(e, part_token(own, p));
        self.touched();
    }

    /// Replace the whole parts section with `parts`, written in the plain
    /// style (`{n}` own, `{cat:n}` foreign). Prefer the incremental edits,
    /// which keep the game's original grouping.
    pub fn set_parts(&mut self, parts: &[PartRef]) {
        let own = self.category();
        let s = self.ensure_parts_section();
        let e = self.parts_end();
        let new: Vec<Tok> = parts.iter().map(|&p| part_token(own, p)).collect();
        self.toks.splice(s..e, new);
        self.touched();
    }
}

fn part_token(own: u32, p: PartRef) -> Tok {
    if p.cat == own {
        Tok::Part { idx: p.idx as u64, val: PartVal::None }
    } else {
        Tok::Part { idx: p.cat as u64, val: PartVal::Single(p.idx as u64) }
    }
}

impl fmt::Display for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for t in &self.toks {
            if !first {
                write!(f, " ")?;
            }
            first = false;
            match t {
                Tok::Sep => write!(f, "|")?,
                Tok::Soft => write!(f, ",")?,
                Tok::Int(v) => write!(f, "{v}")?,
                Tok::Bit(v, _) => write!(f, "{v}")?,
                Tok::Str(s) => write!(f, "{s:?}")?,
                Tok::Part { idx, val } => match val {
                    PartVal::None => write!(f, "{{{idx}}}")?,
                    PartVal::Single(v) => write!(f, "{{{idx}:{v}}}")?,
                    PartVal::List(l) => {
                        let v: Vec<String> = l.values().map(|x| x.to_string()).collect();
                        write!(f, "{{{idx}:[{}]}}", v.join(" "))?
                    }
                },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_samples() {
        for s in [
            "@Ugr$TAm/&nF!e}j^MLp_#G}nY0YSj7e0ss",
            "@UgeA!am/$uk!pu6Lx~WOUOvOs2OwCNqO3g^6O4W/$hHQ~200",
            "@Ugw$Yw5hh0gokbODQHN^OkfE}un5a(OL7hUCLft~0Lk-~o",
            "@Ugv4Ng2}TYg46O/sR610mh77es{X>OBb!rXj6>1j(",
        ] {
            let d = Serial::decode(s).unwrap();
            assert_eq!(d.encode(), s, "{d}");
        }
    }

    #[test]
    fn element_swap() {
        let s = "@Ugv4Ng2}TYg46O/sR610mh77es{X>OBb!rXj6>1j(";
        let mut d = Serial::decode(s).unwrap();
        assert_eq!(d.category(), 17);
        assert_eq!(d.level(), Some(50));
        let mut parts = d.parts();
        let fire = PartRef { cat: 1, idx: 12 };
        assert!(parts.contains(&fire));
        for p in parts.iter_mut() {
            if *p == fire {
                p.idx = 14;
            }
        }
        let n = d.parts().iter().position(|p| *p == fire).unwrap();
        d.replace_part(n, parts[n]);
        let e = d.encode();
        let back = Serial::decode(&e).unwrap();
        assert!(back.parts().contains(&PartRef { cat: 1, idx: 14 }));
        assert_eq!(back.level(), Some(50));
    }
}
