# Weapon item-card stats

Status: CONFIRMED for AR / SMG / pistol; HYPOTHESIS for shotgun, sniper, heavy

Observation (in-game cards, before skills/buffs):
| Item | Dmg | Acc | Reload | FR | Mag | Other | DPS |
|---|---|---|---|---|---|---|---|
| Watching Gomie, Jakobs AR, pearl L42 | 1,122 | 91% | 2.4s | 6.6/s | 18 | crit +96% | 3,915 |
| Ambushing Maggie, Jakobs PS, legendary L42 (mod) | 533 x 6 | 40% | 1.6s | 10.7/s | 10 | crit +96% | 12,709 |
| Plasma Coil, Maliwan SMG, legendary L23 | 92 | 86% | 1.6s | 8.4/s | 40 | 35cm; Shock 191 DMG/s, 8% | 582 |
| Looming Maggie L21-24 (mod) | 125..161 x 6 | 51% | 1.9s | 10.7/s | 8 | crit +58% | 2,273..2,943 |
| Hungry Seeping Abyss, Ripper SR, pearl L30 | 563 | 94% | 2.3s | 11.6/s | 12 | crit +50% | 1,831 |
| Hyperventilating Vamoose, Ripper SR, legendary L45 | 1,827 | 93% | 1.7s | 9.9/s | 5 | Shock 1,052 DMG/s, 20% | 3,019 |

Interpretation (all reproduce the table exactly; tests/card_ground_truth.rs):
- Base damage = 6 x 1.09^L x barrel Damage_Scale; x WeaponType_Init scale, magazine Damage_Scale, element scalar 0.8.
- Stat points: sum of parts' statmodifiers per tag; one point = Weapon_Stats[row].Default x Weapon_Stats[row].<Mfr> (missing = 1) x Rarity_Balance[rarity].Stat_Scale (common 1.0, uncommon 1.2, rare 1.4, epic 1.6, legendary/pearl 1.65); negated when bZeroIsBetter. Rarity has no other damage factor.
- Omitted modifier type = ScaleAdd. Aggregate ((base + PreAdd) x (1 + sum ScaleAdd) / (1 + sum |negative ScaleAdd|)
  x prod ScaleMultiply) + PostAdd, where the divisor takes the negative ScaleAdds of attribute effects only; stat-point
  modifiers stay in the linear sum even when negative (see the Ripper section).
- Omitted NCS table cells read the row struct's default (tools/struct_defaults.json).
- Burst fire rate = N / (N/FR + burst_fire_delay). DPS = game expression weapon_dps_estimate.
- Element DPS = status damage x damage x Status_Application_Defaults[elem].dps / DoT interval 0.33.
- Accuracy = weapon_accuracy_ui_compare: weighted (spread, accuracy impulse, sway) from Weapon_Acc_Uistat_Init; sway term reads 0 (INFERRED: only Sway=0 fits all 4 cards).

Open questions:
- Why sway reads 0 (context-resolution hypothesis).
- Heat/overheat weapons: card_dps differs from the game expression on 37/307 corpus weapons; no in-game reference.
- Name prefixes and item value: see the section below.

Relevant code: crates/bl4core/src/stats.rs, tools/build_db.py.

## Ripper charge and negative ScaleAdd

Status: CONFIRMED on 2 Ripper sniper cards (tests/ripper.rs); parts marked INFERRED below.

- Negative ScaleAdd on an attribute effect divides: x / (1 + |k|). Data: authored values -1 .. -4 (Vamoose/Abyss
  magazine -1, Chuck reload -2, Rainmaker heat -3, Quickdraw equip -1, Goalkeeper charge -2) would zero or negate
  the stat if summed. Cards: Vamoose magazine 10 -> 5, reload 2.87 x (1 - 0.099) / 1.5 = 1.72 -> 1.7s; Abyss
  magazine 15 x 1.2475 / 2 + 2 (PostAdd) = 11.4 -> 12, reload 3.1 x 1.099 / 1.5 = 2.27 -> 2.3s.
- Stat-point modifiers stay linear when negative (INFERRED from cards: dividing them breaks the Gomie, Plasma Coil
  and Maggie accuracy and gives Vamoose 1.8s). Whether two positive attribute-effect ScaleAdds sum or multiply
  is not distinguished by any card (kept summed).
- DPS = weapon_dps_estimate including weapon_compare_charge_time = ChargeTime when MaxChargeStack == 1
  (weapon_is_single_charge). ChargeTime from the part's WeaponBehaviorDef_Charge (Ripper barrels: constant;
  Ripper-licensed magazine: borg_Charge template -> Table_WeaponBorgCharge_Init.ChargeTime_Value, an omitted cell =
  Struct_Weapon_Charge default 1.0); x (1 - fire_rate stat x Charge_Time 0.1 x Borg 1.25 x Stat_Scale).
  Vamoose (no ChargeTime) and Abyss (row/column without a table) both need 1.0: INFERRED class default.
  MaxChargeStack defaults to 1 (INFERRED). Order's OrderCharge and heavy weapons are not included yet.
- Abyss: 563 x 12 / (12 / 11.56 + 2.271 + 0.381) = 1,831; Vamoose: 1,827 x 5 / (5 / 9.854 + 1.724 + 0.794) = 3,019.

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
