# Weapon item-card stats

Status: CONFIRMED for AR / SMG / pistol; HYPOTHESIS for shotgun, sniper, heavy

Observation (in-game cards, before skills/buffs):
| Item | Dmg | Acc | Reload | FR | Mag | Other | DPS |
|---|---|---|---|---|---|---|---|
| Watching Gomie, Jakobs AR, pearl L42 | 1,122 | 91% | 2.4s | 6.6/s | 18 | crit +96% | 3,915 |
| Ambushing Maggie, Jakobs PS, legendary L42 (mod) | 533 x 6 | 40% | 1.6s | 10.7/s | 10 | crit +96% | 12,709 |
| Plasma Coil, Maliwan SMG, legendary L23 | 92 | 86% | 1.6s | 8.4/s | 40 | 35cm; Shock 191 DMG/s, 8% | 582 |
| Looming Maggie L21-24 (mod) | 125..161 x 6 | 51% | 1.9s | 10.7/s | 8 | crit +58% | 2,273..2,943 |

Interpretation (all reproduce the table exactly; tests/card_ground_truth.rs):
- Base damage = 6 x 1.09^L x barrel Damage_Scale; x WeaponType_Init scale, magazine Damage_Scale, element scalar 0.8.
- Stat points: sum of parts' statmodifiers per tag; one point = Weapon_Stats[row].Default x Weapon_Stats[row].<Mfr> (missing = 1) x Rarity_Balance[rarity].Stat_Scale (common 1.0, uncommon 1.2, rare 1.4, epic 1.6, legendary/pearl 1.65); negated when bZeroIsBetter. Rarity has no other damage factor.
- Omitted modifier type = ScaleAdd. Aggregate ((base + PreAdd) x (1 + sum ScaleAdd) x prod ScaleMultiply) + PostAdd.
- Omitted NCS table cells read the row struct's default (tools/struct_defaults.json).
- Burst fire rate = N / (N/FR + burst_fire_delay). DPS = game expression weapon_dps_estimate.
- Element DPS = status damage x damage x Status_Application_Defaults[elem].dps / DoT interval 0.33.
- Accuracy = weapon_accuracy_ui_compare: weighted (spread, accuracy impulse, sway) from Weapon_Acc_Uistat_Init; sway term reads 0 (INFERRED: only Sway=0 fits all 4 cards).

Open questions:
- Why sway reads 0 (context-resolution hypothesis).
- Heat/overheat weapons: card_dps differs from the game expression on 37/307 corpus weapons; no in-game reference.
- Name prefixes and item value: see the section below.

Relevant code: crates/bl4core/src/stats.rs, tools/build_db.py.

## Name prefixes and sell value

Status: CONFIRMED (prefixes, 11/11 in-game names); LIKELY (value, 7 cards within $3)

- Prefix (OakWeaponNamingStrategy): per naming attribute (Resident InventoryNamingAttributes), ratio = Value / BaseValue;
  thresholds (first, second; second < first means lower is better). One passing attribute -> single/double name;
  two or more -> combination name of the top two by single-name priority (Jakobs: Damage 8, Crit 7, FireRate 6,
  Reload 5, Mag 4, Accuracy 3, Elemental 2, ADS 1). Licensed-part prefix from magbarreldatatable (row = barrel-acc
  licence, column = magazine licence). Any part with bDisablePrefixes = no prefix (Plasma Coil).
- Value = itemtype monetaryvalue x prod(part monetaryvaluemodifier); base = 1.12^level x 100 x gun-type multiplier x
  wear factor, wear = 1 - (wear+rust+dirt+sun)/20 drawn from UE FRandomStream(serial seed) within the rarity comp's
  WeaponWearAspect ranges. Off by $0-3 on 6 of 7 cards: residual per-item wear noise, cause UNKNOWN
  (hypothesis: inv_params wear_params 'wb'). Shown as "~$" and for weapons only; other kinds unverified.

Relevant code: stats.rs weapon_prefix / item_value, tests/naming_value.rs.
