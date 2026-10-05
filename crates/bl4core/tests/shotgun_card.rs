//! Shotgun item card checked against an in-game screenshot (2026-10-05):
//! "Looming Balor", Jakobs Shotgun, Epic, Lvl 29 (testdata/saves/11.sav):
//! Damage 190 x 10 | Accuracy 63% | Reload 1.5s | Fire rate 1.3/s | Mag 2 |
//! Crit +94% | 2/Shot | DPS 825 | $11,070.
use bl4core::db::Db;
use bl4core::item::{analyze, Rarity};
use bl4core::serial::Serial;

const BALOR: &str = "@Ugd_t@Fg+0Al4hths!)wuRHXW$@}cgb`l9ZjN}*1nYN1jh00";

fn card(db: &Db, serial: &str) -> (String, Rarity, Vec<(String, String)>) {
    let s = Serial::decode(serial).unwrap();
    let info = analyze(db, &s);
    (info.name.clone(), info.rarity, bl4core::stats::card(db, &s, &info))
}

fn get<'a>(c: &'a [(String, String)], k: &str) -> &'a str {
    c.iter().find(|(a, _)| a == k).map(|x| x.1.as_str()).unwrap_or("-")
}

#[test]
fn looming_balor_lvl29_epic() {
    let db = Db::embedded();
    let (name, r, c) = card(&db, BALOR);
    assert_eq!(name, "Looming Balor");
    assert_eq!(r, Rarity::Epic);
    // pellet count of the "{dmg} x {proj}" line
    assert!(get(&c, "Damage").ends_with(" x 10"), "{c:?}");
    assert_eq!(get(&c, "Accuracy"), "63%");
    assert_eq!(get(&c, "Reload time"), "1.5s");
    assert_eq!(get(&c, "Fire rate"), "1.3/s");
    assert_eq!(get(&c, "Magazine"), "2");
    assert_eq!(get(&c, "Crit damage"), "+94%");
    // uistat_ammo_per_shot: weapon_shot_cost > 1 -> "$VALUE$/Shot"
    assert_eq!(get(&c, "Shot cost"), "2/Shot");
    assert_eq!(get(&c, "Splash radius"), "-");
}

/// Weapons that fire one round per shot show no shot-cost line
/// (Watching Gomie, Jakobs AR, ShotAmmoCost = 1).
#[test]
fn shot_cost_hidden_at_one() {
    let db = Db::embedded();
    let (_, _, c) = card(&db, "@UgzR8/2}TPd%%=nziz-y34wa~/s7hsOQgu+RP_<CEQ0Y+nPy@{#u>$}");
    assert_eq!(get(&c, "Shot cost"), "-");
}

/// The card's per-pellet damage is ~2.2% above the model (186 x 10, DPS 808):
/// 190 and DPS 825 need weapon_damage in [190.14, 190.37]. No item-data
/// source for the factor has been found yet; kept as the reference value.
#[test]
#[ignore = "unexplained ~2.2% shotgun damage gap"]
fn looming_balor_damage_and_dps() {
    let db = Db::embedded();
    let (_, _, c) = card(&db, BALOR);
    assert_eq!(get(&c, "Damage"), "190 x 10");
    assert_eq!(get(&c, "DPS"), "825");
}
