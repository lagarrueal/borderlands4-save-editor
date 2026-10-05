//! Developer tool: decode and validate every item serial in YAML/save files.
//! usage: bl4inspect [--all] <file.yaml|file.sav>...
use bl4core::{db::Db, item, serial::Serial};
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "--all");
    let db = Db::embedded();
    let mut serials = BTreeSet::new();
    for a in args.iter().filter(|a| !a.starts_with("--")) {
        let text = if a.ends_with(".sav") {
            let sid = bl4core::crypto::steam_id_from_path(std::path::Path::new(a)).or_else(|| bl4core::save::known_steam_ids().into_iter().next()).unwrap_or(0);
            String::from_utf8(bl4core::crypto::decrypt(&std::fs::read(a).unwrap(), sid).unwrap()).unwrap()
        } else {
            std::fs::read_to_string(a).unwrap()
        };
        for line in text.lines() {
            if let Some(i) = line.find("serial: '@U") {
                let rest = &line[i + 9..];
                if let Some(j) = rest.find('\'') {
                    serials.insert(rest[..j].to_string());
                }
            }
        }
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut flagged = 0;
    for s in &serials {
        let d = match Serial::decode(s) {
            Ok(d) => d,
            Err(e) => {
                println!("DECODE FAIL {s}: {e}");
                continue;
            }
        };
        let info = item::analyze(&db, &d);
        *by_kind.entry(info.kind.clone()).or_default() += 1;
        let worst = info.issues.iter().map(|i| i.sev).max();
        if verbose || worst >= Some(item::Severity::Warning) {
            if worst >= Some(item::Severity::Warning) {
                flagged += 1;
            }
            println!(
                "{} | {} | {} {} | lvl {:?} | {} | {}",
                info.name,
                info.type_name,
                info.rarity.label(),
                info.element.join("/"),
                info.level,
                d,
                s
            );
            for p in &info.parts {
                println!("    {:>4}:{:<4} {:<18} {:<40} {}", p.r.cat, p.r.idx, p.slot, p.key, p.label);
            }
            for i in &info.issues {
                println!("    [{:?}] {}", i.sev, i.msg);
            }
            if args.iter().any(|a| a == "--stats") {
                for (k, v) in bl4core::stats::card(&db, &d, &info) {
                    println!("    STAT {k}: {v}");
                }
                if args.iter().any(|a| a == "--attrs") {
                    for (n, v, srcs) in bl4core::stats::all_attributes(&db, &d, &info) {
                        println!("      {n} = {v:.4}");
                        for s in srcs {
                            println!("          {s}");
                        }
                    }
                }
            }
        }
        for i in &info.issues {
            let key: String = format!("{:?}: {}", i.sev, i.msg.split(|c: char| c.is_ascii_digit()).next().unwrap_or(""));
            *counts.entry(key).or_default() += 1;
        }
    }
    println!("\n{} unique serials; {} flagged (warning+); kinds {:?}", serials.len(), flagged, by_kind);
    for (k, v) in counts {
        println!("{v:5}  {k}");
    }
}
