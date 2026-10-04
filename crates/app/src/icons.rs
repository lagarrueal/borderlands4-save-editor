//! Item-card icons extracted from the game's UI textures (data/icons.bin).
use std::collections::HashMap;

static PACK: &[u8] = include_bytes!("../../../data/icons.bin");

pub struct Icons {
    map: HashMap<String, &'static [u8]>,
}

impl Icons {
    pub fn load() -> Icons {
        let mut map = HashMap::new();
        let d = PACK;
        if d.len() >= 4 {
            let n = u32::from_le_bytes(d[0..4].try_into().unwrap()) as usize;
            let mut p = 4;
            for _ in 0..n {
                if p + 2 > d.len() {
                    break;
                }
                let nl = u16::from_le_bytes([d[p], d[p + 1]]) as usize;
                p += 2;
                let name = String::from_utf8_lossy(&d[p..p + nl]).to_string();
                p += nl;
                let sz = u32::from_le_bytes(d[p..p + 4].try_into().unwrap()) as usize;
                p += 4;
                map.insert(name, &d[p..p + sz]);
                p += sz;
            }
        }
        Icons { map }
    }

    pub fn get(&self, name: &str) -> Option<(&'static [u8], String)> {
        self.map.get(name).map(|b| (*b, format!("bytes://{name}.png")))
    }

    /// Item-card type silhouette for an item.
    pub fn type_icon(&self, kind: &str, cat_key: &str, mfr: Option<&str>) -> Option<(&'static [u8], String)> {
        let suffix = cat_key.rsplit('_').next().unwrap_or("");
        let name = match kind {
            "weapon" => match suffix {
                "ps" => "type/ico_art_item_card_weap_pistol",
                "sg" => "type/ico_art_item_card_weap_shotgun",
                "ar" => "type/ico_art_item_card_weap_assault",
                "sm" => "type/ico_art_item_card_weap_smg",
                "sr" => "type/ico_art_item_card_weap_sniper",
                _ => "type/ico_art_item_card_weap_pistol",
            },
            "heavy" => match mfr.unwrap_or("") {
                "borg" => "type/ico_art_item_card_heavy_weapon_borg_disc_thrower",
                "maliwan" => "type/ico_art_item_card_heavy_weapon_maliwan_laser",
                "torgue" => "type/ico_art_item_card_heavy_weapon_torgue_rocket_launcher",
                "vladof" => "type/ico_art_item_card_heavy_weapon_vladof_chain_gun",
                _ => "type/ico_art_item_card_heavy_weapon_generic",
            },
            "grenade" => {
                let m = match mfr.unwrap_or("") {
                    "borg" => "borg",
                    "daedalus" => "daedalus",
                    "jakobs" => "jakobs",
                    "maliwan" => "maliwan",
                    "order" => "order",
                    "tediore" => "tediore",
                    "torgue" => "torgue",
                    _ => "vladof",
                };
                return self.get(&format!("type/ico_art_item_card_grenade_{m}"));
            }
            "shield" => "type/ico_art_item_card_energy_shield",
            "repkit" => "type/ico_art_item_card_rep_kit",
            "enhancement" => "type/ico_art_item_card_enhancement",
            "classmod" => "type/ico_art_item_card_class_mod",
            _ => "type/ico_art_item_card_misc_mission",
        };
        self.get(name)
    }

    pub fn manufacturer(&self, mfr: &str) -> Option<(&'static [u8], String)> {
        let m = match mfr {
            "borg" => "ripper",
            other => other,
        };
        self.get(&format!("mfr/ui_art_manu_itemcard_logomark_{m}"))
    }

    pub fn element(&self, label: &str) -> Option<(&'static [u8], String)> {
        let e = match label {
            "Incendiary" => "fire",
            "Shock" => "shock",
            "Corrosive" => "corrosive",
            "Cryo" => "cryo",
            "Radiation" => "radiation",
            _ => "kinetic",
        };
        self.get(&format!("elem/ico_ui_art_elemental_{e}"))
    }

    pub fn firmware(&self, fw: &str) -> Option<(&'static [u8], String)> {
        self.get(&format!("fw/ico_firmware_{}_big", fw.trim_start_matches("fw_")))
    }
}
