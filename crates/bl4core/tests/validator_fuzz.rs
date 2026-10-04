//! Mutate real items and check the validator notices.
use bl4core::db::Db;
use bl4core::item::{analyze, Severity};
use bl4core::serial::{PartRef, Serial};
use std::collections::BTreeSet;

fn corpus() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/yaml");
    let mut out = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    for e in rd.flatten() {
        let s = std::fs::read_to_string(e.path()).unwrap_or_default();
        for line in s.lines() {
            if let Some(i) = line.find("serial: '@U") {
                let rest = &line[i + 9..];
                if let Some(j) = rest.find('\'') {
                    out.insert(rest[..j].to_string());
                }
            }
        }
    }
    out.into_iter().collect()
}

fn worst(db: &Db, s: &Serial) -> Option<Severity> {
    let s2 = Serial::decode(&s.encode()).ok()?;
    analyze(db, &s2).issues.iter().map(|i| i.sev).max()
}

#[test]
fn mutations_are_detected() {
    let db = Db::embedded();
    let c = corpus();
    if c.is_empty() {
        return;
    }
    let (mut n_unknown, mut d_unknown) = (0, 0);
    let (mut n_foreign, mut d_foreign) = (0, 0);
    let (mut n_dup, mut d_dup) = (0, 0);
    let (mut n_cross, mut d_cross) = (0, 0);
    for (k, s) in c.iter().enumerate() {
        let base = Serial::decode(s).unwrap();
        if worst(&db, &base) >= Some(Severity::Warning) {
            continue; // mod items etc.
        }
        let cat = base.category();
        let kind = db.category(cat).map(|c| c.kind.clone()).unwrap_or_default();
        let parts = base.parts();
        let own: Vec<(usize, PartRef)> = parts.iter().copied().enumerate().filter(|(_, p)| p.cat == cat).collect();
        // 1. a part index that does not exist
        if let Some(&(n, p)) = own.first() {
            let mut m = base.clone();
            m.replace_part(n, PartRef { cat: p.cat, idx: 900 + (k as u32 % 50) });
            n_unknown += 1;
            if worst(&db, &m) == Some(Severity::Error) {
                d_unknown += 1;
            }
        }
        // 2. a part from a pool this kind cannot use
        let allowed = bl4core::db::allowed_pools(&kind);
        let pool = [246u32, 1, 243].into_iter().find(|p| !allowed.contains(p)).unwrap();
        let mut m = base.clone();
        m.add_part(PartRef { cat: pool, idx: 10 });
        n_foreign += 1;
        if worst(&db, &m) == Some(Severity::Error) {
            d_foreign += 1;
        }
        // 3. a second barrel / body
        if let Some(&(_, p)) = own.iter().find(|(_, p)| db.part(*p).map(|x| x.s == "barrel").unwrap_or(false)) {
            let other = db.cat_parts[&cat].iter().copied().find(|r| *r != p && db.part(*r).map(|x| x.s == "barrel").unwrap_or(false));
            if let Some(o) = other {
                let mut m = base.clone();
                m.add_part(o);
                n_dup += 1;
                if worst(&db, &m) >= Some(Severity::Warning) {
                    d_dup += 1;
                }
            }
        }
        // 4. a part index from another manufacturer's table (same slot name)
        if kind == "weapon" {
            if let Some(&(n, p)) = own.iter().find(|(_, p)| db.part(*p).map(|x| x.s == "barrel").unwrap_or(false)) {
                let mut m = base.clone();
                // index of a barrel in category cat+1, written as an own-category part
                let foreign_cat = if cat == 2 { 3 } else { 2 };
                if let Some(fb) = db.cat_parts[&foreign_cat].iter().find(|r| db.part(**r).map(|x| x.s == "barrel").unwrap_or(false)) {
                    m.replace_part(n, PartRef { cat: p.cat, idx: fb.idx });
                    n_cross += 1;
                    let w = worst(&db, &m);
                    let same = db.part(PartRef { cat, idx: fb.idx }).map(|x| x.s == "barrel").unwrap_or(false);
                    if same || w >= Some(Severity::Warning) {
                        d_cross += 1;
                    }
                }
            }
        }
    }
    eprintln!("unknown part index: {d_unknown}/{n_unknown}");
    eprintln!("disallowed pool:    {d_foreign}/{n_foreign}");
    eprintln!("duplicate barrel:   {d_dup}/{n_dup}");
    eprintln!("cross-table index:  {d_cross}/{n_cross}");
    assert_eq!(d_unknown, n_unknown);
    assert_eq!(d_foreign, n_foreign);
    assert_eq!(d_dup, n_dup);
}
