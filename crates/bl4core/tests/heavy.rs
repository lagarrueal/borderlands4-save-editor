//! Heavy weapon (Heavy Gun Ordnance) cards checked against in-game screenshots:
//! cooldown, magazine, DPS (weapon_dps_estimate_heavy), body-mod name prefix
//! and value. Values carry the same few-dollar residual as weapons.
use bl4core::db::Db;
use bl4core::item::{analyze, Rarity};
use bl4core::serial::Serial;

struct Card {
    serial: &'static str,
    name: &'static str,
    lines: &'static [(&'static str, &'static str)],
    value: i64,
}

const CARDS: &[Card] = &[
    Card {
        serial: "@Ugr$%Mm/)-_!dOb6IjBVq8;XMf",
        name: "Gamma Void",
        lines: &[
            ("Cooldown", "70s"),
            ("Damage", "1,757"),
            ("Accuracy", "65%"),
            ("Fire rate", "5.0/s"),
            ("Magazine", "1"),
            ("Splash radius", "720cm"),
            ("Radiation", "242 DMG/s | 40% Chance"),
            ("DPS", "8,787"),
        ],
        value: 171063,
    },
    Card {
        serial: "@Ugr$fEm/%P$!bk(PLUrm>VNhdGXHct9=^Fq",
        name: "Junk-Drunk Sidewinder",
        lines: &[
            ("Cooldown", "21s"),
            ("Damage", "1,872"),
            ("Accuracy", "64%"),
            ("Fire rate", "1.5/s"),
            ("Magazine", "5"),
            ("Splash radius", "199cm"),
            ("DPS", "2,809"),
        ],
        // comp_05_legendary_sidewinder's MonetaryValueModifier has PostScale 0.0075
        value: 160,
    },
    Card {
        serial: "@Ugr$fEm/$`s!oWJ9Qm9UiL7hRJLI3",
        name: "Eager Sprezzatura",
        lines: &[
            ("Cooldown", "26s"),
            ("Damage", "2,690"),
            ("Accuracy", "64%"),
            ("Fire rate", "0.6/s"),
            ("Magazine", "2"),
            ("Splash radius", "720cm"),
            ("DPS", "1,614"),
        ],
        value: 11044,
    },
];

#[test]
fn heavy_weapon_cards() {
    let db = Db::embedded();
    for card in CARDS {
        let s = Serial::decode(card.serial).unwrap();
        let info = analyze(&db, &s);
        assert_eq!(info.kind, "heavy");
        assert_eq!(info.rarity, Rarity::Legendary);
        assert_eq!(info.name, card.name, "{}", card.serial);
        let c = bl4core::stats::card(&db, &s, &info);
        // the card lists Cooldown first (heavy_weapon_gadget uistats order)
        assert_eq!(c.first().map(|x| x.0.as_str()), Some("Cooldown"), "{}", card.name);
        for (k, want) in card.lines {
            let got = c.iter().find(|(a, _)| a == k).map(|x| x.1.as_str()).unwrap_or("-");
            assert_eq!(got, *want, "{} {k}", card.name);
        }
        let v = bl4core::stats::item_value(&db, &s, &info).unwrap().round() as i64;
        eprintln!("{}: ${v} (card ${})", card.name, card.value);
        assert!((v - card.value).abs() <= 2, "{}: ${v} vs card ${}", card.name, card.value);
    }
}
