//! Item card numbers checked against in-game screenshots of the same items.
use bl4core::db::Db;
use bl4core::item::{analyze, Rarity};
use bl4core::serial::Serial;

fn card(db: &Db, serial: &str) -> (Rarity, Vec<(String, String)>) {
    let s = Serial::decode(serial).unwrap();
    let info = analyze(db, &s);
    (info.rarity, bl4core::stats::card(db, &s, &info))
}

fn get<'a>(c: &'a [(String, String)], k: &str) -> &'a str {
    c.iter().find(|(a, _)| a == k).map(|x| x.1.as_str()).unwrap_or("-")
}

#[test]
fn watching_gomie_lvl42_pearl() {
    let db = Db::embedded();
    let (r, c) = card(&db, "@UgzR8/2}TPd%%=nziz-y34wa~/s7hsOQgu+RP_<CEQ0Y+nPy@{#u>$}");
    assert_eq!(r, Rarity::Pearlescent);
    assert_eq!(get(&c, "Damage"), "1,122");
    assert_eq!(get(&c, "Fire rate"), "6.6/s");
    assert_eq!(get(&c, "Magazine"), "18");
    assert_eq!(get(&c, "Reload time"), "2.4s");
    assert_eq!(get(&c, "DPS"), "3,915");
    assert_eq!(get(&c, "Crit damage"), "+96%");
}

#[test]
fn plasma_coil_lvl23_legendary() {
    let db = Db::embedded();
    let (r, c) = card(&db, "@Ugw$Yw3FZbO%(f5uhbq+2p?;{BsFtWs%/YctokI-^00");
    assert_eq!(r, Rarity::Legendary);
    assert_eq!(get(&c, "Damage"), "92");
    assert_eq!(get(&c, "Fire rate"), "8.4/s");
    assert_eq!(get(&c, "Magazine"), "40");
    assert_eq!(get(&c, "Reload time"), "1.6s");
    assert_eq!(get(&c, "DPS"), "582");
    assert_eq!(get(&c, "Shock"), "191 DMG/s | 8% Chance");
}

/// The Maggie only exists with the JakobsMasherMaggie mod; skip without it.
#[test]
fn ambushing_maggie_lvl42_modded() {
    let db = Db::embedded();
    if !db.meta.mod_paks.iter().any(|p| p.contains("Maggie")) {
        return;
    }
    let (_, c) = card(&db, "@UgbV{rFj^2{Xjjl_RG}Jms6?ekwL`5#r9+)Vb?OeP6zUXe7HTB/");
    assert_eq!(get(&c, "Damage"), "533 x 6");
    assert_eq!(get(&c, "Fire rate"), "10.7/s");
    assert_eq!(get(&c, "Magazine"), "10");
    assert_eq!(get(&c, "Reload time"), "1.6s");
    assert_eq!(get(&c, "DPS"), "12,709");
    assert_eq!(get(&c, "Crit damage"), "+96%");
}
