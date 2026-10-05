//! Crypto and YAML round trips over every test save.
use bl4core::{crypto, yaml};

/// Steam ID of the test saves: BL4_STEAM_ID, else the first one with BL4 saves on this PC.
fn sid() -> u64 {
    std::env::var("BL4_STEAM_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| bl4core::save::known_steam_ids().into_iter().next())
        .unwrap_or(0)
}
// files the game wrote itself (the others were written by older tools)
const GAME_WRITTEN: &[&str] = &["1", "10", "11", "12", "2", "3", "3_20260907_201632", "4", "5", "6", "7", "9", "profile"];

fn dir(sub: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(sub)
}

#[test]
fn decrypt_encrypt_yaml_roundtrip() {
    let Ok(rd) = std::fs::read_dir(dir("saves")) else { return };
    let mut n = 0;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "sav").unwrap_or(true) {
            continue;
        }
        let stem = p.file_stem().unwrap().to_str().unwrap().to_string();
        let data = std::fs::read(&p).unwrap();
        let y = crypto::decrypt(&data, sid()).unwrap_or_else(|e| panic!("{stem}: {e}"));
        let text = String::from_utf8(y.clone()).unwrap();
        let doc = yaml::parse(&text).unwrap_or_else(|e| panic!("{stem}: {e}"));
        let out = doc.emit();
        let game = GAME_WRITTEN.contains(&stem.as_str());
        if !game {
            // tool-written: style may differ, but the content must survive
            assert_eq!(yaml::parse(&out).unwrap(), doc, "{stem}");
        } else if out != text {
            let a: Vec<&str> = text.split('\n').collect();
            let b: Vec<&str> = out.split('\n').collect();
            let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            panic!("{stem}: yaml differs at line {}: {:?} vs {:?}", i + 1, a.get(i), b.get(i));
        }
        let enc = crypto::encrypt(out.as_bytes(), sid()).unwrap();
        if GAME_WRITTEN.contains(&stem.as_str()) {
            assert!(enc == data, "{stem}: re-encrypted bytes differ from the game's");
        }
        assert_eq!(crypto::decrypt(&enc, sid()).unwrap(), out.as_bytes());
        n += 1;
    }
    eprintln!("{n} saves round-tripped");
    assert!(n > 0);
}
