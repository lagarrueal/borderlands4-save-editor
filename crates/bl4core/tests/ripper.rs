//! Ripper sniper cards ("charge before Full Auto firing") checked against
//! in-game screenshots: negative ScaleAdd attribute effects divide (magazine
//! -1 = half, reload -0.5 = / 1.5) and the DPS includes the charge-up time
//! (weapon_compare_charge_time).
use bl4core::db::Db;
use bl4core::item::{analyze, Rarity};
use bl4core::serial::Serial;
use bl4core::stats::{card, compute, UiEval};

fn get<'a>(c: &'a [(String, String)], k: &str) -> &'a str {
    c.iter().find(|(a, _)| a == k).map(|x| x.1.as_str()).unwrap_or("-")
}

/// Card lines, plus the game's own weapon_dps_estimate evaluated from its
/// expression, which must agree with the card DPS.
fn lines(db: &Db, serial: &str) -> (Rarity, Vec<(String, String)>, f64, f64) {
    let s = Serial::decode(serial).unwrap();
    let info = analyze(db, &s);
    let c = compute(db, &s, &info);
    let ui = UiEval::new(db, &info, &c).get("weapon_dps_estimate");
    (info.rarity, card(db, &s, &info), ui, c.card_dps().unwrap_or(0.0))
}

#[test]
fn hungry_seeping_abyss_lvl30_pearl() {
    let db = Db::embedded();
    let (r, c, ui, dps) = lines(&db, "@Ugxp/&3C0H^OzscbhbmN}7L^b664eq_6V<6Ys9UIVsDVb0)By");
    assert_eq!(r, Rarity::Pearlescent);
    assert_eq!(get(&c, "Damage"), "563");
    assert_eq!(get(&c, "Accuracy"), "94%");
    assert_eq!(get(&c, "Reload time"), "2.3s");
    assert_eq!(get(&c, "Fire rate"), "11.6/s");
    assert_eq!(get(&c, "Magazine"), "12");
    assert_eq!(get(&c, "Crit damage"), "+50%");
    assert_eq!(get(&c, "DPS"), "1,831");
    assert!((ui - dps).abs() < 0.5, "game expression {ui} vs card {dps}");
}

#[test]
fn hyperventilating_vamoose_lvl45_legendary() {
    let db = Db::embedded();
    let (r, c, ui, dps) = lines(&db, "@Ugxp/&38o7oOuq>lRG}I*bZ8A~CTdf4P`gm=5C");
    assert_eq!(r, Rarity::Legendary);
    assert_eq!(get(&c, "Damage"), "1,827");
    assert_eq!(get(&c, "Accuracy"), "93%");
    assert_eq!(get(&c, "Reload time"), "1.7s");
    assert_eq!(get(&c, "Fire rate"), "9.9/s");
    assert_eq!(get(&c, "Magazine"), "5");
    assert_eq!(get(&c, "Crit damage"), "+50%");
    assert_eq!(get(&c, "Shock"), "1,052 DMG/s | 20% Chance");
    assert_eq!(get(&c, "DPS"), "3,019");
    assert!((ui - dps).abs() < 0.5, "game expression {ui} vs card {dps}");
}
