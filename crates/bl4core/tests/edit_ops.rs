//! Every edit the editor offers, applied to copies of real saves, then written,
//! re-read and checked against the game's own structural rules.
use bl4core::db::Db;
use bl4core::save::{self, Character, Container, Profile, SaveFile, SaveKind};
use bl4core::serial::Serial;
use std::path::PathBuf;

/// Steam ID of the test saves: BL4_STEAM_ID, else the first one with BL4 saves on this PC.
fn sid() -> u64 {
    std::env::var("BL4_STEAM_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| bl4core::save::known_steam_ids().into_iter().next())
        .unwrap_or(0)
}

fn copy(name: &str, tag: &str) -> Option<PathBuf> {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/saves").join(name);
    if !src.exists() {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("bl4edit_test_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dst = dir.join(name);
    std::fs::copy(&src, &dst).unwrap();
    Some(dst)
}

fn reopen(p: &PathBuf) -> SaveFile {
    SaveFile::open(p, Some(sid())).unwrap()
}

#[test]
fn character_edits_roundtrip() {
    let Some(p) = copy("11.sav", "char") else { return };
    let db = Db::embedded();
    let mut sf = SaveFile::open(&p, Some(sid())).unwrap();
    let inv0 = save::check_invariants(&sf.doc, SaveKind::Character);
    assert!(inv0.is_empty(), "{inv0:?}");
    let backup = sf.make_backup().unwrap();
    assert!(backup.exists());

    {
        let mut c = Character(&mut sf.doc);
        c.set_level("Character", 60).unwrap();
        c.set_level("Specialization", 100).unwrap();
        c.set_currency("cash", 99_999_999);
        c.set_currency("eridium", 50_000);
        c.set_difficulty("Hard");
        c.set_true_mode(true);
        c.unlock_uvh(7, 3);
        c.set_ammo("pistol", 900);
        c.unlock_equip_slots();
        c.set_cosmetic("vehicle", "Cosmetics_Vehicle", Some("Cosmetics_Vehicle_Mat16_PolePosition"));
        c.set_name("Edited");
    }
    let mut sets = std::collections::HashMap::new();
    for (m, info) in &db.missions {
        if !info.set.is_empty() {
            sets.entry(info.set.to_lowercase()).or_insert_with(Vec::new).push(m.clone());
        }
    }
    save::complete_story(&mut sf.doc, &sets);

    // edit an equipped item, add one, remove one in the middle, remove an equipped one
    let items = save::list_items(&sf.doc);
    let eq = items.iter().find(|i| i.container == Container::Backpack && i.flags == Some(1)).unwrap().clone();
    let mut s = Serial::decode(&eq.serial).unwrap();
    if s.set_level(55) {
        save::set_item_serial(&mut sf.doc, &eq, &s.encode());
    }
    let n_before = save::list_items(&sf.doc).iter().filter(|i| i.container == Container::Backpack).count();
    save::add_backpack_item(&mut sf.doc, &eq.serial);
    let after = save::list_items(&sf.doc);
    assert_eq!(after.iter().filter(|i| i.container == Container::Backpack).count(), n_before + 1);
    let mid = after.iter().filter(|i| i.container == Container::Backpack && i.flags != Some(1)).nth(3).unwrap().clone();
    save::remove_item(&mut sf.doc, &mid);
    let eq2 = save::list_items(&sf.doc).into_iter().find(|i| i.container == Container::Backpack && i.flags == Some(1)).unwrap();
    save::remove_item(&mut sf.doc, &eq2);

    let inv = save::check_invariants(&sf.doc, SaveKind::Character);
    assert!(inv.is_empty(), "{inv:?}");
    sf.write().unwrap();

    let mut back = reopen(&p);
    {
        let c = Character(&mut back.doc);
        assert_eq!(c.name(), "Edited");
        assert_eq!(c.xp("Character").unwrap(), (60, save::min_xp(60, save::CHAR_XP_MULT)));
        assert_eq!(c.currency("cash").as_deref(), Some("99999999"));
        assert_eq!(c.uvh(), (7, 3));
        assert!(c.story_complete());
    }
    assert!(save::check_invariants(&back.doc, SaveKind::Character).is_empty());
}

#[test]
fn profile_edits_roundtrip() {
    let Some(p) = copy("profile.sav", "prof") else { return };
    let db = Db::embedded();
    let mut sf = SaveFile::open(&p, Some(sid())).unwrap();
    assert_eq!(sf.kind, SaveKind::Profile);
    {
        let mut pr = Profile(&mut sf.doc);
        let nodes: Vec<(String, u32)> = db.sdu.iter().map(|n| (n.name.clone(), n.cost)).collect();
        pr.set_sdu_nodes(&nodes);
        assert_eq!(pr.sdu_nodes().len(), db.sdu.len());
        assert!(pr.sdu_tokens() >= 3225);
        let ids: Vec<String> = db.cosmetics.iter().filter(|(_, c)| c.group == "Unlockable_Echo4").map(|(k, _)| k.clone()).collect();
        pr.add_unlockables("unlockable_echo4", &ids);
        pr.add_shared_progress("shared_progress.vault_hunter_level");
    }
    let bank_before = save::bank_used(&sf.doc);
    let first = save::list_items(&sf.doc).into_iter().find(|i| i.container == Container::Bank).unwrap();
    save::add_bank_item(&mut sf.doc, &first.serial);
    save::remove_item(&mut sf.doc, &first);
    assert_eq!(save::bank_used(&sf.doc), bank_before);
    let inv = save::check_invariants(&sf.doc, SaveKind::Profile);
    assert!(inv.is_empty(), "{inv:?}");
    sf.write().unwrap();
    let mut back = reopen(&p);
    assert!(Profile(&mut back.doc).sdu_tokens() >= 3225);
    assert!(save::check_invariants(&back.doc, SaveKind::Profile).is_empty());
}

#[test]
fn unmodified_save_writes_identical_bytes() {
    let Some(p) = copy("11.sav", "ident") else { return };
    let before = std::fs::read(&p).unwrap();
    let mut sf = SaveFile::open(&p, Some(sid())).unwrap();
    assert!(!sf.is_modified());
    sf.write().unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), before);
}

#[test]
fn built_items_decode_and_have_no_errors() {
    let db = Db::embedded();
    let mut bad = vec![];
    let mut n = 0;
    for c in db.item_categories() {
        let comps: Vec<String> = db
            .cat_parts
            .get(&c.id)
            .map(|l| l.iter().filter_map(|r| db.part(*r)).filter(|p| p.s == "inv_comp").map(|p| p.k.clone()).collect())
            .unwrap_or_default();
        for comp in comps {
            let Some(s) = bl4core::item::build_item(&db, c.id, &comp, 50, 1234) else {
                bad.push(format!("{} {comp}: build failed", c.key));
                continue;
            };
            n += 1;
            let info = bl4core::item::analyze(&db, &s);
            let errs: Vec<String> = info.issues.iter().filter(|i| i.sev == bl4core::item::Severity::Error).map(|i| i.msg.clone()).collect();
            if !errs.is_empty() {
                bad.push(format!("{} {comp}: {errs:?}", c.key));
            }
        }
    }
    eprintln!("{n} items built, {} with errors", bad.len());
    for b in bad.iter().take(20) {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}

#[test]
fn scale_backpack_to_level() {
    let Some(p) = copy("11.sav", "scale") else { return };
    let mut sf = SaveFile::open(&p, Some(sid())).unwrap();
    let (changed, _skipped) = save::scale_items(&mut sf.doc, Container::Backpack, 37);
    assert!(changed > 0);
    for it in save::list_items(&sf.doc) {
        if matches!(it.container, Container::Backpack | Container::Equipped) {
            let s = Serial::decode(&it.serial).unwrap();
            if let Some(l) = s.level() {
                assert_eq!(l, 37, "{:?} {}", it.container, it.slot);
            }
        }
    }
    let inv = save::check_invariants(&sf.doc, SaveKind::Character);
    assert!(inv.is_empty(), "{inv:?}");
}
