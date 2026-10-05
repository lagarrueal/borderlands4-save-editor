//! Full weapon names (stat + licensed-part prefixes) and card values ($)
//! checked against in-game screenshots. Values are within a few dollars:
//! the wear model is 1e-4 off (see docs).
use bl4core::db::Db;
use bl4core::item::analyze;
use bl4core::serial::Serial;

const CARDS: &[(&str, &str, i64, bool)] = &[
    ("@UgzR8/2}TPd%%=nziz-y34wa~/s7hsOQgu+RP_<CEQ0Y+nPy@{#u>$}", "Watching Gomie", 122089, false),
    ("@Ugw$Yw3FZbO%(f5uhbq+2p?;{BsFtWs%/YctokI-^00", "Plasma Coil", 7888, false),
    ("@UgbV{rFj^2{Xjjl_RG}Jms6?ekwL`5#r9+)Vb?OeP6zUXe7HTB/", "Ambushing Maggie", 82462, true),
    ("@UgbV{rFg_4rEJo02RG}7?s6(YjokOKVZR!r{6si>J7HS*<", "Looming Maggie", 15465, true),
    ("@UgbV{rFg_4rWP(nk3e~7XEh<r/QR`6WQ0Y*ex`Qf(I)$o*8ixP", "Ambushing Maggie", 17161, true),
    ("@UgbV{rFg_4ryid?+RG/`es75U+HEJEI9V#6v4C)T*6si>J7OEuz", "Cooking Ambushing Maggie", 20572, true),
    ("@UgbV{rFg_4ru7O6Q3bm+2J*pk*9BNZ}P@zz>P`eN", "Terrifying Cuca", 8744, false),
];

#[test]
fn names_and_values() {
    let db = Db::embedded();
    let modded = db.meta.mod_paks.iter().any(|p| p.contains("Maggie"));
    for (serial, name, value, needs_mod) in CARDS {
        if *needs_mod && !modded {
            continue;
        }
        let s = Serial::decode(serial).unwrap();
        let info = analyze(&db, &s);
        assert_eq!(info.name, *name, "{serial}");
        let v = bl4core::stats::item_value(&db, &s, &info).unwrap().round() as i64;
        eprintln!("{name}: ${v} (card ${value})");
        assert!((v - value).abs() <= 4, "{name}: ${v} vs card ${value}");
    }
}
