//! Level caps and the XP curve come from the game's xp_progression data;
//! a game patch that changes them must fail here, not silently corrupt saves.
use bl4core::db::Db;
use bl4core::save::{self, Character, SaveFile, SaveKind};

fn sid() -> u64 {
    std::env::var("BL4_STEAM_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| save::known_steam_ids().into_iter().next())
        .unwrap_or(0)
}

#[test]
fn caps_and_curve_match_game_data() {
    let db = Db::embedded();
    for (key, cap, mult) in [
        ("oak2_characterxp_progression", save::MAX_CHAR_LEVEL, save::CHAR_XP_MULT),
        ("oak2_specializationxp_progression", save::MAX_SPEC_LEVEL, save::SPEC_XP_MULT),
    ] {
        let p = db.xp.get(key).unwrap_or_else(|| panic!("{key} missing from the database"));
        assert_eq!(p.levelcap, cap, "{key} level cap changed in the game data");
        // every segment up to the cap uses the formula min_xp() implements
        for f in &p.functions {
            assert_eq!(f.get("multiplier").copied(), Some(mult), "{key}");
            assert_eq!(f.get("power").copied(), Some(2.8), "{key}");
            assert_eq!(f.get("offset").copied(), Some(7.33), "{key}");
        }
        // the last segment continues past its maxlevel (observed: a game save
        // at specialization 110 holds XP within min_xp(110)..min_xp(111))
        assert!(!p.functions.is_empty(), "{key}: no curve");
    }
}

/// Levels above the old cap of 60 are written, saved and re-read cleanly.
#[test]
fn character_level_above_60_roundtrips() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/saves/11.sav");
    if !src.exists() {
        return;
    }
    for level in [61, save::MAX_CHAR_LEVEL] {
        let dir = std::env::temp_dir().join(format!("bl4edit_lvl_{}_{level}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("11.sav");
        std::fs::copy(&src, &p).unwrap();
        let mut sf = SaveFile::open(&p, Some(sid())).unwrap();
        Character(&mut sf.doc).set_level("Character", level).unwrap();
        assert!(save::check_invariants(&sf.doc, SaveKind::Character).is_empty());
        sf.write().unwrap();
        let mut back = SaveFile::open(&p, Some(sid())).unwrap();
        let (l, pts) = Character(&mut back.doc).xp("Character").unwrap();
        assert_eq!(l, level);
        assert_eq!(pts, save::min_xp(level, save::CHAR_XP_MULT));
        assert_eq!(back.doc.get("progression.point_pools.characterprogresspoints").and_then(|n| n.as_i64()), Some(level as i64 - 1));
        assert!(save::check_invariants(&back.doc, SaveKind::Character).is_empty());
        // beyond the cap is clamped, never written
        Character(&mut back.doc).set_level("Character", save::MAX_CHAR_LEVEL + 5).unwrap();
        assert_eq!(Character(&mut back.doc).xp("Character").unwrap().0, save::MAX_CHAR_LEVEL);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
