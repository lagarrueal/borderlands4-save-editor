# Heavy weapon (Heavy Gun Ordnance) item cards

Status: CONFIRMED (cooldown, magazine, DPS, prefixes on 3 in-game cards); LIKELY (value, within $2)

Observation (in-game cards):
| Item | Cooldown | Dmg | Acc | FR | Mag | Radius | Element | DPS | Value |
|---|---|---|---|---|---|---|---|---|---|
| Gamma Void, Maliwan, legendary L48 | 70s | 1,757 | 65% | 5.0/s | 1 | 720cm | Radiation 242 DMG/s, 40% | 8,787 | $171,063 |
| Junk-Drunk Sidewinder, Torgue, legendary L30 | 21s | 1,872 | 64% | 1.5/s | 5 | 199cm | - | 2,809 | $160 |
| Eager Sprezzatura, Torgue, legendary L25 | 26s | 2,690 | 64% | 0.6/s | 2 | 720cm | - | 1,614 | $11,044 |

Interpretation (tests/heavy.rs):
- Lines: heavy_weapon_gadget uistats (bInheritBaseUIStats false), uistat_gadget_cooldown first.
- Cooldown = Gadget_Cooldown (CooldownTime): barrel hw_cooldown_attr Cooldown x comp Gadget_HW_Rarity.Cooldown_Scale
  x (1 - cooldown_reduction stat points); "$VALUE$s", no precision set -> whole number.
  part_barrel_javelin lists hw_cooldown_attr without a row: the template's own row (Gadget_HW_Barrels TOR_Barrel_01,
  50) applies (Sprezzatura 50 x 0.7525 x 0.7 = 26.3). Torgue part_barrel_01 relies on the same rule for all its values.
- Magazine = weapon_max_loaded_ammo from hw_barrel_attr_base_values MagazineSize_Value; Unique_HW_Barrels.MAL_GammaVoid
  omits it -> row struct default 1 (Gamma Void "1"). Both rules are applied to heavy items only (stats.rs Eval.own_rows).
- DPS = weapon_dps_estimate_heavy = (Damage x MagSize) / (MagSize / FireRate), Damage = ordnance_damage_compare
  (= weapon_damage_per_shot); no reload term.
- Prefix: OakHeavyWeaponNamingStrategy.BodyPrefixModNameDataTable (e.g. TorgueHeavyWeaponNamingBodyPrefix); body
  accessory tags body_mod_a..d: one tag -> its row, column SingleOrNoMod ("Eager" = body_mod_c); several -> row = all
  but the last joined with '.', column Mod<last letter> ("Junk-Drunk" = body_mod_a.body_mod_b / ModC). The
  multi-tag rule is INFERRED from the table shape (row X only has columns for letters after X), verified on 3 tags.
- Value = attr_calc_price_heavy_weapon = 1.12^L x Economy_BaseCosts.HeavyWeapon (165) x wear,
  x parts' MonetaryValueModifier (rarity 3.75, HW_Body_Acc 1.05 each, element 1.2). Sidewinder is $160 because
  comp_05_legendary_sidewinder's MonetaryValueModifier has PostScale 0.0075 (game data).
- Wear: INFERRED factor 1 for heavies although their comps carry weapon_wear_epic (drawn wear gives Gamma Void
  $168,498; wear 1 gives $171,065). Same +$1-2 residual as weapons remains.

Open questions:
- Why heavies read no wear (HeavyWeaponGadget class; no data source found).
- Barrel names of non-unique heavies (BarrelOne/BarrelTwoModNameDataTable, barrel_mod_* tags) are not implemented;
  those items still show "<Mfr> Heavy Gun".
- Card line order after Cooldown follows the weapon order (radius/element/DPS order differs from the game).
