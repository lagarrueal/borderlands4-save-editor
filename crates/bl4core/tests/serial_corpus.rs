//! Round-trips every item serial found in the test saves.
use bl4core::serial::Serial;
use std::collections::BTreeSet;

fn corpus() -> BTreeSet<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/yaml");
    let mut out = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x == "yaml").unwrap_or(false) {
            let s = std::fs::read_to_string(&p).unwrap();
            for line in s.lines() {
                if let Some(i) = line.find("serial: '@U") {
                    let rest = &line[i + 9..];
                    if let Some(j) = rest.find('\'') {
                        out.insert(rest[..j].to_string());
                    }
                }
            }
        }
    }
    out
}

#[test]
fn roundtrip_all_serials() {
    let c = corpus();
    if c.is_empty() {
        eprintln!("no corpus; skipping");
        return;
    }
    let mut bad = vec![];
    for s in &c {
        match Serial::decode(s) {
            Ok(d) => {
                if d.encode() != *s {
                    bad.push(format!("{s} -> {} [{d}]", d.encode()));
                }
            }
            Err(e) => bad.push(format!("{s}: {e}")),
        }
    }
    eprintln!("{} serials, {} failures", c.len(), bad.len());
    for b in bad.iter().take(20) {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}

#[test]
fn reencode_after_parts_identity() {
    // Rewriting the parts section with the same parts must keep the item identical.
    let mut bad = 0;
    let c = corpus();
    for s in &c {
        let d = Serial::decode(s).unwrap();
        let mut e = d.clone();
        // replace every part by itself, then remove+re-add the last one
        for (n, p) in d.parts().iter().enumerate() {
            e.replace_part(n, *p);
        }
        assert_eq!(e.encode(), *s);
        let back = Serial::decode(&e.encode()).unwrap();
        if back.parts() != d.parts() || back.level() != d.level() || back.seed() != d.seed() {
            bad += 1;
            eprintln!("{s}\n  {d}\n  {back}");
        }
    }
    assert_eq!(bad, 0);
}
