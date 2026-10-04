# BL4 save fields: what the editor reads and writes

Topic: **save-fields**. Researched 2026-10-04 against game build `compatible_changelist: 4845623`. The game was running during this work, so it was all done offline on copies.

- **Inputs:** 16 character saves and `profile.sav` in `testdata/saves`, plus their decrypted YAML in `testdata/yaml`.
- **Upstream code:** `third_party/bl4` (monokrome/bl4 v0.8.5). The fresh release build is at `third_party/bl4/target/release/bl4.exe`.
- **Game data:** the newest NCS tables, already carved into `research/ncs_extract/base/*.ncs`, decoded with `bl4 ncs show <file> --json`.
- **Scratch work:** `C:\Users\alexa\AppData\Local\Temp\claude\bl4editor-research\save-fields\`
- **Reusable outputs:** `research/save-fields-data/`, listed in §10.

Every claim below is marked **[measured]** (run or diffed here), **[game data]** (read from NCS), **[upstream]** (read in monokrome code) or **[inferred]**.

---

## 0. Summary

1. **The container format is fully known and can be reproduced byte for byte [measured].**
   - The format is AES-256-ECB, then PKCS7 padding, then a zlib stream (`78 9C`, level 6), then a **u32 LE uncompressed length**. That length is a 4-byte trailer after the zlib stream.
   - Re-encrypting the decrypted YAML this way reproduced **13 of 13 game-written `.sav` files exactly**.
   - Upstream `bl4` writes `78 DA` (level 9) with an 8-byte trailer (adler32 LE plus length LE). Its output is not identical to the game's, but the game accepts it (§7.3).
   - In Rust, only `flate2` with the **`zlib` feature** (C zlib through libz-sys) gives identical bytes. The `miniz_oxide` and `zlib-rs` backends do not.
2. **The YAML uses a small subset of YAML, and its style can be reproduced exactly [measured].**
   - The emitter is about 40 lines (`save-fields-data/emit.py`). It works on a tree that keeps each scalar's raw text and quote style.
   - It reproduced all 13 game-written YAML files byte for byte.
   - `serde_yaml` (upstream) keeps the meaning but rewrites formatting: it drops trailing spaces, turns empty values into `null`, `TRUE` into `true` and `.002` into `0.002`, and drops some quotes. On profile.sav that is 373 changed lines; on 11.sav it is 854 changed lines.
3. **UVH lives in the character save: `globals.highest_unlocked_vault_hunter_level` (0..7) and `globals.vault_hunter_level` (0 = off, 1..7) [measured + game data].**
   - The rank-up challenge stats `stats.challenge.mission_uvh_*`, `uvh_N_finalchallenge` and `stats.dlc_challenge.uvh_7` should be set as well, to stay consistent.
   - UVH 7 exists in the game data (defeat Subjugator or Thol the Invincible in UVH 6). No save has reached it yet.
   - **Evidence of a revert:** a tool-edited save (`4_20260426_203929.sav`: UVH 6, +22,000,000 cash, UVH challenges added) was overwritten by the game 3 minutes later (`4.sav`: UVH 1, cash back, challenges removed). Playtime was unchanged in between. This fits the game's in-memory copy of the character overwriting the edit.
   - **Never write a save while `Borderlands4.exe` is running.**
4. **SDU upgrades and SDU tokens are account-wide now, stored in profile.sav [measured + game data].**
   - The upgrades are under `domains.local.progression_shared.graphs[name=sdu_upgrades]`.
   - The tokens are `domains.local.progression_shared.point_pools.echotokenprogresspoints`.
   - The full node list and costs are extracted (§5.4). Buying everything costs **3225** tokens.
   - Weapon slots 3/4, repair kit, enhancement and class mod slots are per character, in `state.unique_rewards` and `state.inventory.equip_slots_unlocked`.
5. **Golden keys cannot be edited offline [game data].** `golden_key: shift` is a marker. The currency def has `bmanagedbyshift: true`, so the server holds the value.
6. **Cosmetics: the equipped ones are in `state.gbxactorparts.*`; the unlocked ones are in profile `domains.local.unlockables.<group>.entries` [measured].**
   - A catalogue of **1303 cosmetic unlockable IDs**, with display names, was built from the GbxActorPart, inv_custom, GbxActor and hover_drive NCS tables.
   - The profile currently has about 620 of them.
7. **The XP formula is verified on all game-written saves [measured]:** `minXP(L) = floor(m * (L^2.8 + 7.33))`, with m = 60 for Character and m = 80 for Specialization; level 1 needs 0 XP.
   - The NCS level cap says Character 70 and Specialization 701.
   - **In practice the cap is 60:** save 6 has XP above the level-70 threshold and is still level 60.
8. **Upstream `bl4` has several stale or wrong parts for current saves [measured]:**
   - `--map reveal` fails: the fog-of-war data moved to the profile.
   - `save set` cannot create missing keys, and still writes a `.bak` when it fails.
   - The `state_flags` bit-9 ("not equipped") documentation does not match real saves.
   - The bundled `missions.tsv` covers 320 of the 661 missions in the NCS data.

---

## 1. Method and commands

```bash
# decrypt (identical to testdata/yaml for all files)
third_party/bl4/target/release/bl4.exe save X.sav -s 76561198112570585 decrypt -o X.yaml
# upstream round trip (backup is forced on: `-b/--backup` takes no value, so `--backup=false` is an error)
bl4.exe save 11.rt.sav -s 76561198112570585 set state.currencies.cash 24507508   # writes 11.rt.sav.bak + .bak.json
bl4.exe save 10.ms2.sav -s ... missions set missionset_main_grasslands2a -y
bl4.exe save 11.sav -s ... missions list main|side|all
bl4.exe ncs show research/ncs_extract/base/<table>.ncs --json > table.json   # works on the compressed NCS too
python crypt.py / reenc.py / emit.py / pipeline_test.py   (scratch dir; copies in research/save-fields-data)
```

PyYAML needs a constructor for the custom tag (`yaml.SafeLoader.add_constructor('!tags', ...)`).

**PyYAML is YAML 1.1. It turns the game's `Off` into `False`, and it also accepts `TRUE`.** Use it for analysis only, never for writing. The game treats `Off`, `On` and `None` as plain strings.

---

## 2. Container format [measured]

| Layer | Detail |
|---|---|
| Key | `BASE_KEY` (32 bytes, `third_party/bl4/src/bl4/src/crypto.rs`). The first 8 bytes are XORed with the Steam ID as a u64 little-endian. Steam ID here: `76561198112570585`. It is also the save folder name. |
| Cipher | AES-256-ECB over the whole file. The size is always a multiple of 16. |
| Padding | PKCS7. All 16 game files have valid PKCS7 (pad 1..16). |
| Compression | zlib, header `78 9C`, **level 6** (Python `zlib.compress(y, 6)` reproduces it exactly). |
| Trailer | **4 bytes: u32 LE uncompressed YAML length**, between the zlib stream and the padding. |
| Upstream writer | `encrypt_sav()` writes level 9 (`78 DA`) and appends **adler32 LE plus length LE** (8 bytes). One file written by another tool (`STBX-c4sh-2.sav`) has `78 9C` with the same 8-byte trailer. |

Results of `crypt.py` / `reenc.py` on every test save:

```
game-written (789c, 4-byte trailer):  1,10,11,12,2,3,3_2026..,4,5,6,7,9,profile   -> zlib(6)+len4 == original bytes
tool-written:  4_20260426_203929 (78da, adler+len), 8 (78da, adler+len), STBX-c4sh-2 (789c, adler+len)
```

**Rust test** (scratch `zrt/`, separate `CARGO_TARGET_DIR`, flate2 1.1.10):

| flate2 backend | Byte-identical to game? |
|---|---|
| `rust_backend` (miniz_oxide) | no (output 10736 bytes vs 10912 for 11.sav) |
| `zlib-rs` | no |
| **`zlib` (libz-sys, C zlib built with mingw gcc)** | **yes, all 13 game files** |

When decrypting, accept either trailer: read the zlib stream and ignore what follows it. When writing, use the game's form (level 6 plus the 4-byte length).

---

## 3. The YAML dialect and round-trip fidelity [measured]

The game's YAML uses only these features. This was surveyed with `yaml.parse` over all 16 files.

- **Collections:** block mappings and block sequences only. No flow style, anchors, aliases, comments, multi-line scalars or double quotes.
- **Scalars:**
  - Plain: 45,321 of them.
  - Single-quoted: 976. They are quoted when the string starts with `@` (all item serials), looks like a negative number (`'-1'`, `'-206660.84375'`), contains `,` `[` `:` or `'`, or looks numeric (`'1'`). Inside quotes, `'` is written as `''`.
- **Tag:** one custom tag, `!tags`, on some sequences (pips lists, `vaultcard_purchases`). It is written as `key: !tags` followed by the list.
- **Booleans:** `true`/`false` in some places and `TRUE`/`FALSE` in others (`globals`, world facts). **Keep the original casing.**
- **Floats:** written by the game as-is, for example `.002` and `205675.140625`.
- **Key order:** differs from save to save, even at the top level. Keep it.

Exact formatting rules (byte-identical on all 13 game files):

```
mapping entry, scalar value :  "<indent><key>: <scalar>"
mapping entry, empty/null   :  "<indent><key>: "            <- trailing space, nothing after
mapping entry, map value    :  "<indent><key>: "            <- trailing space, children at indent+2
mapping entry, seq value    :  "<indent><key>: "  (or "<indent><key>: !tags")  children at SAME indent:  "<indent>- item"
seq item that is a mapping  :  "<indent>- <firstkey>: ..." then other keys at indent+2
indent = 2 spaces; LF line ends; NO newline at end of file
```

**What the upstream `serde_yaml` round trip changes.** Measured: `bl4 save set` with an unchanged value, then decrypt and diff.

| File | Lines changed | Kinds of change |
|---|---|---|
| 11.sav | 854 (of 1679) | trailing spaces removed, empty → `null`, `TRUE`/`FALSE` → `true`/`false`, `.002` → `0.002`, quotes dropped on `gap,...` strings, final newline added |
| profile.sav | 373 | 350 trailing spaces, 23 empty → `null` |

In both files, `!tags`, key order, serial quoting and the `'-123.4'` strings survived. **The game loads this re-serialized form** (§7.3), so these changes are cosmetic. A byte-exact writer is still better, because unedited files then come out identical and diffs stay clean.

**Recommendation for the Rust editor.**
- Write a small lossless parser for this subset. Each scalar node keeps its raw text, its quote style (none or single) and an optional tag; mappings keep their order.
- Emit with the rules above.
- Validate in CI: parse then emit must give the identical bytes for every file in `testdata/yaml`.
- For a new scalar, quote it with `'...'` if it is empty, starts with an indicator (`@ - ! & * [ { ' " % | > # ?`), contains `: ` `,` `[` `]` `{` `}` `'` or ` #`, or would read as a number, bool or null. Otherwise write it plain.
- New booleans: use the casing of the siblings (`globals` uses `TRUE`/`FALSE`; `state` uses `true`/`false`).

---

## 4. File inventory

Folder: `Documents\My Games\Borderlands 4\Saved\SaveGames\<steamid>\Profiles\client\`

- **profile.sav sits in `client\` next to the slots**, not in `Profiles\` as upstream docs say.
- Slots are `<N>.sav` (1..12 here).
- Other files in the folder came from tools: `8.sav.bak`, `8.sav.bak.json` and `9.sav.bak*` (upstream smart backup), and a `backup\` folder (another tool).
- `STBX-c4sh-2.sav` and `3_20260907_201632.sav` are copies. **[inferred]** The game only loads `<N>.sav`.
- `steam_autocloud.vdf` is present in the SteamID folder, so **Steam Cloud is enabled** for this game.
- **Cloned slots work [measured]:** slots 4, 5 and 7 are all "Amon" with the same `char_guid` and `save_game_header.guid`, and 5 and 7 were played separately. The same holds for the C4SH copies in slots 2, 3 and 6.
- Game process: `Borderlands4.exe`, path `OakGame\Binaries\Win64\Borderlands4.exe`. The game rewrote `11.sav` and `profile.sav` together at 16:21 during this session.
- Profile ↔ slot link: `domains.local.characters_selected: C_11` names the last selected slot (the file `11.sav`). `domains.local.characters` has `c_1..c_12` keys, all null.

**Character top-level keys:** `state`, `globals`, `stats`, `missions`, `progression`, `unlockables` (only in some files), `onlinecharacterprefs`, `pips`, `gbx_discovery_pc`, `gbx_discovery_pg`, `activities`, `oak.ui.progression_data`, `save_game_header`, `world_state`, `timed_facts`. The order varies.

**Profile top-level keys:** `inputprefs`, `ui`, `onlineprefs`, `domains`, `echoprefs`, `ui_screen_data`, `audioprefs`, `deep_freeze_pips`, `oak.ui.news_data`, `save_game_header`, `oak.ui.dlc_data`, `mapviewerprefs`, `oak.ui.bigmap_data`, `pips`.

---

## 5. Field reference

Key: C = character save, P = profile. "Seen" values come from the 16 test files.

### 5.1 Identity and headers

| Path | File | Type | Example / seen | Safe range and notes |
|---|---|---|---|---|
| `state.char_guid` | C | 32 upper-case hex chars | `3B67116641774DAB955F2AB41769A961` | Identity; at runtime it is `OakPlayerState.ActiveCharGuid`. **Do not change.** Copies of a character share it. |
| `state.class` | C | str | `Char_DarkSiren`, `Char_Paladin`, `Char_ExoSoldier`, `Char_Gravitar`, `Char_RoboDealer`, `Char_CorpoHacker` | Read only. Many class-specific trees and cosmetics depend on it. |
| `state.char_name` | C | str | `Vex`, `C4SH`, `Loveless` | Free text. Quote it if it contains special characters. |
| `save_game_header.guid` | C/P | 32 hex | `8E302EA0...` | Constant per character across copies and edits. Do not change. |
| `save_game_header.timestamp` | C/P | unix seconds (UTC) | `1791123444` | Equals `state.last_played_timestamp` at write time. Upstream does not update it and the game still accepts the save. Optionally set it to now. |
| `save_game_header.original_platform` | C/P | str | `STEAM` | Missing in saves older than about May 2026. |
| `save_game_header.compatible_changelist` | C/P | int | `4845623` (newest), `4830603`, `4772434`, `4709277` | Build that last wrote the file. Leave it. |
| `state.last_played_timestamp` | C | unix | `1791123444` | |
| `state.first_timestamp` | C | null | empty | |
| `state.total_playtime` | C | float seconds | `205675.140625` | Cosmetic. |

### 5.2 Difficulty, UVH and True Mode

| Path | File | Type | Seen | Safe range and notes |
|---|---|---|---|---|
| `state.player_difficulty` | C | enum str | `Normal` | `Easy` \| `Normal` \| `Hard` (UI strings `Difficulty_Easy/Normal/Hard` in display_data NCS). A Hardcore table exists but no save uses it. |
| `state.true_mode` | C | bool, lowercase | `false` | True Mode: enemies scale as for a 4-player party. Available after the campaign. |
| `globals.true_mode_override` | C | bool, `FALSE` | `FALSE` | Missing in older saves. |
| `globals.highest_unlocked_vault_hunter_level` | C | int | 1, 6; **missing** in saves that never unlocked UVH (1, 10, 11, 12) | **0..7.** UVH 7 rank-up exists in challenge0 NCS. UI tooltips run to UVH 10, but there are no challenges past 7. |
| `globals.vault_hunter_level` | C | int | 0, 1, 2, 6 | **0 = UVH off**; 1..`highest_unlocked`. Mission conditions test `Global.vault_hunter_level != 0`. |
| `globals.highest_unlocked_mayhem_level` | C | int | 0 | Mayhem tables exist in the data but the mode is not released. Leave at 0. |
| `stats.challenge.mission_uvh_{1a,1b,1c,2a..2d,3a..3d,4a..4d,5a..5c,6a}` | C | int 1 | `1` | Rank-up sub-challenges (§6.1). |
| `stats.challenge.uvh_{1..5}_finalchallenge` | C | int 1 | `1` | Rank-up final challenges. |
| `stats.dlc_challenge.uvh_7` | C | int 1 | (never seen) | UVH 7 rank-up. The whole `stats.dlc_challenge` map is new. |
| `stats.achievements.03_uvh_5` | C | int 1 | `1` | Achievement flag. |
| `stats.tutorial.uvh_unlock`, `uvh_unlock2` | C | int 1 | `1` | Tutorial-seen flags. |
| `missions.local_sets.missionset_main_postgame.missions.micro_uvh_{blackmarkettutorial,firmwaretransfertutorial,trueboss,trait}` | C | mission map | `status: completed` | The UVH 1 unlock missions. |
| `domains.local.unlockables.shared_progress.entries[]` | P | str list | `shared_progress.vault_hunter_level`, `.prologue_completed`, `.story_completed`, `.epilogue_started` | Account-wide progress flags. They likely enable skip options for new characters. |
| `deep_freeze_pips.pips_list_deep_freeze` (`!tags`) | P | str list | `profile.newgame.ultimatevaulthunter`, `profile.DLC.*` | UI "new" markers and DLC ownership markers. Do not add DLC markers. |

### 5.3 Money and currencies

| Path | File | Type | Seen max | Safe range and notes |
|---|---|---|---|---|
| `state.currencies.cash` | C | int | 173,100,289 | 0..2,147,483,647. **[inferred]** UE int32. Not verified in game; 999,999,999 is a conservative ceiling. |
| `state.currencies.eridium` | C | int | 25,899 | Same as cash. |
| `state.currencies.golden_key` | C | literal `shift` | `shift` | **Not editable.** The currency def `golden_key` has `bmanagedbyshift: true` (Capital4 NCS), so SHiFT holds the value. Show it read-only. |
| `domains.local.shared.currencies.vaultcard0{1..5}_tokens` | P | int | 217 | Vault Card tokens, account-wide (`sourcetype: Account`). |
| `domains.local.shared.experience[]` `{type: VaultCard0N_Experience, level, points}` | P | int | lvl 134 | Vault Card XP. The curve is `Oak2_VaultCardXP_Progression` (8·L^2.25, base 2, cap 9999). Lower priority. |
| `domains.local.progression_shared.point_pools.echotokenprogresspoints` | P | int | 3715 | **SDU tokens**, total earned. Available = this minus the costs of purchased SDU nodes. Buying everything needs at least 3225. |
| `progression.point_pools.echotokenprogresspoints` | C | int | 3715 (save 1 only) | Left over from before shared progression (April 2026). Ignore it. |

### 5.4 SDU upgrades: the full node list [game data, `progress_graph0.ncs` → `sdu_upgrades`]

The graph uses point pool `EchoTokenProgressPoints` (profile, `profilesavetype: Shared`) with `busemaxprogresspointsasactivationcost: true`. **A node's `points_spent` in the save equals its cost.** Each node needs the previous one (`noderefname`).

| Group (nodes) | Costs per level | Effect per level | Total |
|---|---|---|---|
| `Ammo_Pistol_01..07` | 5, 10, 20, 30, 50, 80, 120 | `ammo_pistol_maxvalue` +100 | 315 |
| `Ammo_SMG_01..07` | same | `ammo_smg_maxvalue` +180 | 315 |
| `Ammo_AR_01..07` | same | `ammo_assaultrifle_maxvalue` +140 | 315 |
| `Ammo_SG_01..07` | same | `ammo_shotgun_maxvalue` +20 | 315 |
| `Ammo_SR_01..07` | same | `ammo_sniper_maxvalue` +20 | 315 |
| `Backpack_01..08` | 5, 10, 20, 30, 50, 80, 120, **235** | `backpack_max_size` +4, +4, +6, +6, +6, +8, +8, +12 (= +54) | 550 |
| `Bank_01..08` | same | `bank_max_size` +25, +50, +50, +50, +50, +50, +100, +100 (= +475) | 550 |
| `Lost_Loot_01..08` | same | `lost_loot_storage_maxsize` +1 each (= +8) | 550 |
| **All** | | | **3225** |
| `Weapon_Slot_03` | condition: player level ≥ 5 | reward `pgraph.sdu_upgrades.Weapon_Slot_03` → equipslot **2** | per character |
| `Weapon_Slot_04` | level ≥ 8 | → equipslot **3** | per character |
| `RepKit_Slot` | fact `Global.repkit_unlocked == True` | → equipslot **6** | per character |
| `Enhancement_Slot` | level ≥ 15 | → equipslot **7** | per character |
| `Class_Mod_Slot` | level ≥ 20 | → equipslot **8** | per character |

How the save stores it:

```yaml
# profile.sav
domains:
  local:
    progression_shared:
      graphs:
      - name: sdu_upgrades
        group_def_name: Oak2_GlobalProgressGraph_Group
        nodes:
        - name: Ammo_Pistol_01
          points_spent: 5         # = cost
        ...
      point_pools:
        echotokenprogresspoints: 3715
# character: slots granted as unique rewards + list of unlocked equip slots
state:
  unique_rewards: [..., pgraph.sdu_upgrades.Weapon_Slot_03, pgraph.sdu_upgrades.Weapon_Slot_04,
                   pgraph.sdu_upgrades.RepKit_Slot, pgraph.sdu_upgrades.Enhancement_Slot, pgraph.sdu_upgrades.Class_Mod_Slot]
  inventory:
    equip_slots_unlocked: [2, 3, 6, 7, 8]       # level-5 char: [2, 6]
```

- **Ammo maxima [inferred from saves at full SDU]:** the base values are pistol 200, SMG 360, AR 280, SG 80, SR 50. With all SDUs they reach **900, 1620, 1260, 220, 190**, which matches save 3. `repairkit` is 10 in every save.
- **Backpack capacity: the base value is NOT in NCS.** It is the `BackpackContainer.MaxSize` default in a UE asset; the attribute resolves to `BackpackContainer.MaxSize`. Data points:
  - 11.sav holds 50 items (9 of them equipped) at current SDUs, with none lost.
  - Save 1 (April) held 66.
  - Memory notes say an Amon character "holds 36", which conflicts with the +54. **Unresolved, needs an in-game test.**
  - Items over capacity are **deleted on load**. The editor must count before adding items, and fail safe by refusing to add past the count the save already holds.
- **Bank:** 200 items in `profile.sav`. The base capacity is unknown, as for the backpack.
- **Lost Loot:** `state.lostloot.slots` is 6 or 14 (6 + 8 Lost_Loot SDUs). Save 11 holds 26 items with `slots: 14`, so `slots` is not a simple item cap.

### 5.5 Levels, XP and points

| Path | File | Type | Seen | Safe range and notes |
|---|---|---|---|---|
| `state.experience[0]` `{type: Character, level, points}` | C | ints | L60 / 5714893 | Level **1..60**. The NCS cap of 70 does not apply in practice: save 6 has 28.5M XP and is still level 60. `points` must be at least `minXP(level)`. |
| `state.experience[1]` `{type: Specialization, level, points}` | C | ints | L110 / 42289219 | Level 1..701 (NCS `levelcap 701`). Starts locked until `unlocks.epilogue_started`. |
| `progression.point_pools.characterprogresspoints` | C | int | 59 at L60 | **Equals Character level − 1** in all game-written saves. The game grants it per level (`GbxProgressionBehaviorData_AddPointsEachLevel`). It is the *total* granted; unspent points are derived. |
| `progression.point_pools.specializationtokenpool` | C | int | 109 at spec 110 | About spec level − 1 (one save has spec 16 with 16). Missing for characters that never unlocked specialization. |
| `progression.progress_state_data.{characterprogresspoints,specializationtokenpool}` `{id: GUID, reset_count}` | C | | `reset_count: 0..2` | Respec bookkeeping. Leave it. |

**XP formula [measured, verified on all 12 consistent game-written saves; table in `save-fields-data/xp_table.tsv`]:**

```
minXP(L) = 0                                  if L == 1
         = floor(m * (L^2.8 + 7.33))          m = 60 (Character), 80 (Specialization)
```

NCS source: `Oak2_CharacterXP_Progression` (`GbxExperienceFunction_Exponential` with multiplier, power 2.8 and offset 7.33).

| Level | Char min XP | Spec min XP |
|---|---|---|
| 2 | 857 | 1143 |
| 30 | 820962 | 1094617 |
| 50 | **3430227** (= saves capped at the old level 50) | 4573636 |
| 60 | **5714893** (= every save sitting at the level-60 cap) | 7619858 |
| 70 | 8799286 | 11732382 |

- The upstream `experience_progression.tsv` gives `60·L^2.8` without the offset, so it is 439 too low.
- The upstream doc's "L2 = 1100" is wrong.
- **Level is not recomputed downward [upstream + measured]:** save 10 is level 5 with 1840 XP, a leftover from an earlier curve mod. To set a level, write **both** `level` and `points = minXP(level)`.
- Keep `characterprogresspoints = level − 1` at the same time.
- **Never exceed level 60:** memory notes report broken saves when the level is above the cap.

### 5.6 Skill trees

`progression.graphs[]` (character) holds entries `{name, group_def_name, nodes: [...]}`. Each node is `{name, points_spent | is_activated, activation_level}`.

- **Node `name` is the display alias**, for example `Established Gameplay Loop` or `Phase Echo`, not an internal ID.
- **Graph names by class:**
  - CorpoHacker: `Progress_Corpohacker_*` with group `ProgressGroup_Corpohacker`.
  - DarkSiren: `Progress_DS_*` and `Progress_DarkSiren_*`.
  - Paladin: `Progress_PLD_*` and `Progress_Paladin_*`.
  - ExoSoldier: `Progress_EXO_*` and `progress_graph_exo_*` (group `progress_group_exo`).
  - Gravitar: `Progress_Grav_*` and `progress_graph_grav_*`.
  - RoboDealer: `Progress_robo_*`.
- **Shared across classes:** `ProgressGraph_Specializations`, with nodes Adventurer, Wanderer, Artificer, Slayer, Survivor, Enforcer and Hunter, max 100 each, from pool `specializationtokenpool`. Also `ProgressGraph_Specializations_Skills` (`ST_n_Skill_m`, `is_activated`, `activation_level`).
- Action skills and augments are `is_activated: true` nodes.
- **Consistency rule [measured]:** the sum of `points_spent` over a class's trunk and branch graphs is at most `characterprogresspoints`. In saves 1 and 11 it is exactly equal.
- **Definitions:** all 134 graphs are extracted to `save-fields-data/progress_graphs.json` (from `progress_graph0/4/6` NCS). Per-node `maxprogresspoints` is often empty and inherited from the `progress_graph_passives_template*` graphs.
- **Recommendation:** for v1, offer level edits plus "reset skills" (empty the class graphs; the game refunds the points). Fine-grained skill editing can wait.

### 5.7 Cosmetics

| Path | File | Type | Example | Notes |
|---|---|---|---|---|
| `state.gbxactorparts.character` | C | str | `'gap,Cosmetics_DarkSiren_Head[Cosmetics_DarkSiren_Head07_Demon],Cosmetics_DarkSiren_Skin[Cosmetics_DarkSiren_SkinFilter28_Ritual]'` | Format: `gap,` then a comma-separated list of `<SlotPart>[<CosmeticPart>]`. Slots: `Cosmetics_<Class>_Body`, `_Head`, `_Skin`. A missing slot means the default. Slot-name casing varies (`Cosmetics_Robodealer_Body` holds `Cosmetics_RoboDealer_Body02_Premium`), so **copy the casing from the catalogue**. The game always single-quotes this value. |
| `state.gbxactorparts.echo4` | C | str | `'gap,Cosmetics_Echo4_Attachment[Cosmetics_Echo4_Attachment17],cosmetics_echo4_body[Cosmetics_Echo4_Body03_Ripper],Cosmetics_Echo4_Skin[Cosmetics_Echo4_Skin61_FullMetal]'` | ECHO-4 drone. The skin slot appears as both `cosmetics_echo4_skin` and `Cosmetics_Echo4_Skin`. |
| `state.gbxactorparts.vehicle` | C | str | `'gap,Cosmetics_Vehicle[Cosmetics_Vehicle_Mat16_PolePosition]'` | Vehicle skin. |
| `state.personal_vehicle` | C | str | `PV_DarkSiren`, `PV_Borg`, `PV_CityOrder`, `PV_Grazer` | Selected vehicle. |
| `state.hover_drive` | C | str | `HoverDrive_Daedalus_05` | Selected hover drive. |
| `state.player_customization` | C | null | empty | Unused. |
| `domains.local.unlockables.unlockable_<class>.entries[]` | P | str list | `Unlockable_DarkSiren.Head07_Demon` | **Unlocked character cosmetics, account-wide.** One group per class: darksiren, paladin, exosoldier, gravitar, robodealer, corpohacker. |
| `domains.local.unlockables.unlockable_echo4 / unlockable_weapons / unlockable_vehicles / unlockable_hoverdrives` | P | str list | `Unlockable_Weapons.Mat27_GoldenPower`, `Unlockable_Vehicles.Borg`, `Unlockable_HoverDrives.Jakobs_01` | ECHO-4, weapon skins, vehicles plus vehicle skins, hover drives. |
| `unlockables.unlockable_hoverdrives.entries[]` | C | str list | `Unlockable_HoverDrives.Jakobs_01` | Per-character hover drive unlock (`unlockable_hoverdrives_character` in the data). Present in only 7 saves. |
| `oak.ui.dlc_data.ui_dlc_data.vaultcard_purchases` (`!tags`) | P | str list | `Unlockable_DarkSiren.Head21_Reindeeer` | Cosmetics bought from Vault Cards. |
| `pips.pips_list` (`!tags`) | C/P | str list | `profile.charactercustomization.Cosmetics_Corpohacker_Head.Head24_GetTheHorns` | "New item" badges. Optional. |
| `state.unique_rewards[]`, `state.packages[]` | C | | `RewardPackage_CharacterSkin_37_Maliwan`, `Reward_HoverDrive_Jakobs_01` | Reward bookkeeping (already granted / pending in mail). Leave it. |
| `stats.achievements.04_cosmetics_collect` | C | int | 252 | Achievement counter only. |

**Catalogue:** `save-fields-data/cosmetics_catalogue.json`, mapping each unlockable ID to `{group, part, name, serialindex, source}`, 1303 entries. How it was built:
- `GbxActorPart0/4/6` entries with `"unlockable": "unlockable'Unlockable_X.Y'"` give the class, ECHO-4 and vehicle-skin parts, with `gbxactorpart` = the name used in `gbxactorparts`. The display name is the third comma field of `description` (for example `Yabai`).
- `inv_custom0/4` give `Unlockable_Weapons.*` (240).
- `GbxActor0/4/6` `unlockedby` give the vehicles.
- `hover_drive0/6` `unlockedby` / `unlockedbycharacter` give the hover drives.

Counts:

| Group | In game data | In profile |
|---|---|---|
| CorpoHacker | 127 | 77 |
| DarkSiren | 125 | 78 |
| Paladin | 125 | 78 |
| RoboDealer | 126 | 80 |
| ExoSoldier | 124 | 78 |
| Gravitar | 121 | 78 |
| Echo4 | 116 | 64 |
| Vehicles | 119 | 52 |
| Weapons | 240 | 41 |
| HoverDrives | 39 (+40 per-character) | 19 |

`Unlockable_Vehicles.Stingray` is in the profile but not found in the data scanned; it is probably in an online-patch table.

**What "unlock all cosmetics" requires:**
- Append the missing IDs to the matching `domains.local.unlockables.<group>.entries` lists in the profile.
- Creating a missing group map is a structural insert.
- Optionally add `pips` entries.

Caveats:
1. Some catalogue entries may be unreleased or test content (check `serialindex.status == Active`).
2. Entitlement-tied items (`*_PreOrder`, `*_Premium`, `GoldenPower`, `SHiFT`, `HeadHunter`, `Legacy`, and DLC `Vault Card` cosmetics) belong to paid or online entitlements. Upstream `entitlements.rs` detects ownership from exactly these markers. Default to excluding them.

To change what is equipped, edit the `gbxactorparts` string, using only parts whose unlockable is in the profile.

### 5.8 Missions

| Path | File | Type | Values | Notes |
|---|---|---|---|---|
| `missions.tracked_missions` | C | str list, ≤ 3 | `Mission_Main_City1`, `none`, `none`; may be missing | UI tracking. Padded with the literal `none`. |
| `missions.local_sets.<missionset_id>` | C | map | | Lower-case set ID, for example `missionset_main_beach`. 114 sets exist (missionset0/4/6 NCS; the upstream `mission_sets.tsv` matches). |
| `…<set>.status` | C | str or absent | `completed` | Absent while the set is active or partial. |
| `…<set>.cursorposition` | C | int | 1, 3 | Index of the current mission in the chain. |
| `…<set>.missions.<mission_id>.status` | C | str | `completed` (1358), `Active` (32), `Available` (11), `Kickoffing` (1) | Mission IDs are lower case. 661 missions exist in Mission0/4/6 NCS; upstream `missions.tsv` has 320 of them. |
| `…missions.<m>.ui_flags` | C | int | `1` | "Seen" flag. |
| `…missions.<m>.mission_giver_ui_flags` | C | int | | |
| `…missions.<m>.final.<objective>_endstate` | C | str/bool | `completed` (1555), `TRUE` (311), `deactivated` (126) | End-state of each objective branch. The game writes these even for its own story-skip (save 7). |
| `…missions.<m>.objectives.<objective>.status` | C | str | `WaitingOnMission`, `WaitingOnDependencies`, `Active`, `Completed_Finishing`, `Completed_PostFinished` | Only for active missions. **These are the in-mission checkpoints.** Objective IDs are lower-cased `objective_sets[].objectives[].objective` from the Mission NCS. |
| `…missions.<m>.exit` | C | str | `Exit`, `Micro_UVH_FirmwareTransferTutorial_SHA_Exit` | Which exit point was taken. |
| `…missions.<m>.state.entrypoint` | C | int | 3 | |
| `missions.remote_sets` | C | | (never present) | Upstream handles it; it does not appear in current saves. |
| `state.checkpoint_name` | C | str | `World_P.RS_City_GRA_To_SHA_Transition`, `World_P.FT_SHA_ZanesBar`, `Raid2_P.RS_Raid2_BossRepsawn` | **Respawn / load checkpoint** (`<Map>_P.<Station>`, prefixes RS_, FT_, FTS_, RTS_). Only use values seen in saves; an invalid one probably spawns at the map default **[inferred]**. |
| `state.world_region_name`, `gbx_discovery_pc.metrics.{lastworld,lastregion}` | C | str | `KairosGeneric` / `World_P` / `city_RegionA` | Must stay consistent with `checkpoint_name`. |
| `activities.allactivities[]` | C | list | `serial: 'act,5,ContractActivity_...'`, `missionstate` | Contracts. Preserve. |
| `globals.*` | C | | see below | **Mission side effects.** The editor must set these when completing missions (§6.3). |

**Mission display names:** from Mission NCS `ux_display.text`, taking the part after the second `, `; for example `NexusSerialized, 64C96…, Recruitment Drive`. The set-to-mission map, `missiontype` and objective count are in `save-fields-data/missions_catalogue.json`. 373 of the 661 missions have no mission set (dynamic events, contracts); they never appear under `local_sets`.

**Main story chain [game data + save 7]:**

| Set | Mission(s) and display name |
|---|---|
| prisonprologue | Guns Blazing |
| beach | Recruitment Drive |
| grasslands1 | Down and Outbound |
| grasslands2a | `mission_main_grasslands2`, A Lot to Process |
| grasslands2b | One Fell Swoop |
| mountains1 | Shadow of the Mountain |
| shatteredlands1 | `mission_main_shatteredlands1` Wrath of the Ripper Queen + `…1b` Siege and Destroy |
| grasslands3 | Rush the Gate |
| mountains2a | `mission_main_mountains2`, Crystal Brawl |
| shatteredlands2 | Unpaid Tab |
| searchforlilith | Rising Action (needs all three regional chains) |
| mountains2b | Dark Subject |
| shatteredlands3 | Her Flaming Vision |
| elpis | Another Day, Another Universe |
| mountains3 | His Vile Sanctum |
| city1 | The Falling Wall |
| city1_b | Means of Ascent |
| city2 | Plan Z |
| city3 | The Timekeeper's Order |
| cityepilogue | Secrets of the Vault |
| main_postgame | `micro_uvh_*` (UVH 1 unlock) |

DLC sets: `main_cowbell_unlock`/`main_cowbell` (5 missions), `main_harmonica0..4`, `dlc_cello`, `dlc_banjo`, `dlc_harp`, `dlc_mandolin`, `dlc_viola`, `dlc_tuba`, `dlc_raid1`, `dlc_raid2`.

**The game's own "complete story" recipe** is `profile_skip_reward_def0.ncs` → `UltimateVaultHunterSkip` (reward level 30). It lists every main mission plus `Grasslands_Side_MurderMystery_Beckon` and the four `Micro_UVH_*`. Save `7.sav` ("Throwaway Amon", L30) is a **game-generated example** of that state: every main set is `status: completed`, each with one `mission_*: {status: completed, final: {...}}`, plus completed zone activities. Use it as the template for "complete story". There are also `SkipToDLC_Cowbell` and `SkipToDLC_Harmonica` defs.

### 5.9 Inventory

| Path | File | Type | Notes |
|---|---|---|---|
| `state.inventory.items.backpack.slot_<N>` | C | map `{serial, flags?, state_flags?}` | N is 0..count−1 and **contiguous** in every save. The serial is a single-quoted `@Ug…` string. |
| `state.inventory.items.backpack.unknown_items` | C | list of `{serial}` | Appears only after the game parks items whose parts are missing (memory note; not in the current test data). Preserve it. |
| `state.inventory.equipped_inventory.equipped.slot_<K>` | C | **list** with one map `[{serial, flags: 1, state_flags}]` | K: 0–3 weapons, **4 shield, 5 ordnance (grenade or heavy weapon), 6 repair kit, 7 enhancement, 8 class mod** [measured by decoding all equipped serials]. Empty or locked slots are absent. |
| `state.inventory.equip_slots_unlocked` | C | int list | `[2, 3, 6, 7, 8]`, the optional slots (see the SDU table). |
| `state.inventory.active_slot` | C | int | Selected weapon slot, 0..3. |
| `state.lostloot` `{items: [{serial, in_machine: bool}], slots: int}` | C | | `items` may be empty (null). |
| `domains.local.shared.inventory.items.bank.slot_<N>` | P | `{serial, state_flags}` | 200 items, slots 0..199. **No `flags` key.** |
| `state.ammo.{assaultrifle,pistol,shotgun,smg,sniper,repairkit}` | C | int | See the ammo maxima in §5.4. |

**Invariants measured on game-written saves.** Write all of them.
1. Every equipped serial also appears in the backpack, with **identical `state_flags`**: 110 of 110. (`STBX-c4sh-2`, written by a tool, breaks this.)
2. **`flags: 1` marks equipped items.** Every equipped item has it in both places. Unequipped backpack items normally have no `flags` key; 4 exceptions were seen.
3. `state_flags` values seen:
   - backpack: 1 (most), 3, 5, 4, 513, 515, 517, or absent (43 items);
   - bank: 1, 3, 513, 515.
   
   Bit 0 = 1, bit 1 (2) = favourite, bit 2 (4) = junk, bits 4–7 = labels 1–4 (upstream `state_flags.rs`). **Bit 9 (512) is NOT "not equipped".** Equipped items carry 513 and 515 too. Its meaning is unknown, so keep it as it is. New backpack items should use `state_flags: 1` with no `flags` key, like the 184 ordinary items. (Upstream `StateFlags::backpack()` = 513 and `add_backpack_item` writes `flags: 0`; neither matches what the game writes.)
4. Use the next free slot number, and stay at or below capacity (§5.4).

### 5.10 Other sections (preserve; rarely edited)

- `globals` (C), all keys seen: `time_of_day`, `introlights1-4`, `movegrant_glide`, `movegrant_grapplegrabber`, `movegrant_grapplegrabber_presentation`, `movegrant_echolocation`, `movegrant_ordonitegloves`, `repkit_unlocked`, `prologue_completed`, `mainmissioncomplete`, `lockdownlifted`, `worldchange_has_sight`, `event_count_*`, `city_burritochallenge_*`, `event_colosseum_visited`, `player_skipped_to_dlc`, `any_hover_drive_unlocked`, `has_started_cowbell`, `seen_movie_cowbell_p`, `riftboss_entrystation`, plus the UVH and true-mode keys above.
- `stats` (C) sub-maps: `achievements`, `challenge` (about 175 counters), `openworld` (`activities`, `collectibles`, `misc`), `regions`, `tutorial`, `shinygear`, `cello_*`, `cowbell_*`, `harmonica_challenges`, `daily_*`, `weekly_*`, `hud`, `spawncontrol`, and `dlc_challenge` (UVH 7).
- `state.challenge_objectives` (C): `[{challenge_ident, challenge_bitfield: [u32]}]`.
- `state.blackmarket_cooldown` (C): unix timestamp.
- `state.seen_eridium_logs` (C): bitmask (262143).
- `state.last_death_location_x|y|z`, `last_death_discovery_world` (C).
- `state.using_shared_progression: true` (C).
- `world_state`, `timed_facts`, `gbx_discovery_pc` (pins, metrics), `gbx_discovery_pg` (C).
- **Fog of war is now in the profile:** `domains.local.gbx_discovery_pc_shared.{saveid, fodsaveversion: 2, foddatas[{levelname, foddimensionx/y: 128, compressiontype: Zlib, foddata: base64(zlib(128×128 bytes))}]}`, 18 maps.
- `domains.local.vault_cards` (P): `activated_card`, `day_reset_key`, `week_reset_key`, `vault_cards[{name, challenges_daily{active, picked}, challenges_weekly}]`.
- Black market in the profile: `blackmarket_items[{blackmarket_itemtype: MAL_SR, blackmarket_itemcomp, blackmarket_gamestage}]`, `blackmarket_usedpools`, `blackmarket_trackerstate`, `last_seen_bmvm_idx`.
- `domains.local.shared.merged_shared_data_version: 8` (P). `domains.local.stats.regions.*.discovered` (P).
- `domains.local.unlockables.echo_upgrade_challenges` (197), `echo_log_challenges` (197), `vault_object_challenges` (18), `sharedprogress_cello`, `sharedprogress_cowbell` (P): account-wide collectible and activity completion, and the **sources of SDU tokens** **[inferred]**.
- Settings in the profile (`inputprefs`, `ui`, `audioprefs`, …): never touch them. Memory notes say a crash-reporter reset wipes the whole Config folder; the saves are separate.

---

## 6. How to do each requested feature

### 6.1 UVH unlock (a)

Rank-up chain [challenge0/6 NCS]. Each rank's stat must be complete, and each rank requires the previous final challenge.

| Unlocks | Sub-challenge stats (`stats.challenge.*`) | Final stat | Done in |
|---|---|---|---|
| UVH 1 | `mission_uvh_1a` (Zane firmware tutorial), `1b` (black market), `1c` (Big Encore / true boss); `mission_uvh_1d` (Wildcard trait mission, a newer DLC chunk; never seen in saves) | `uvh_1_finalchallenge` (`micro_uvh_trait`) | story done (`Challenge_Misc_World_CompleteCityEpilogue`) |
| UVH 2 | `2a` Bramblesong, `2b` Bio-Thresher Omega, `2c` Bio-Bulkhead, `2d` Sydney Pointylegs | `uvh_2_finalchallenge` (Rush the Gate wildcard) | UVH 1 |
| UVH 3 | `3a` Shadowpelt, `3b` Pangolin boss, `3c` Keeper, `3d` Frank the Furnace | `uvh_3_finalchallenge` | UVH 2 |
| UVH 4 | `4a` Battle Wagon, `4b` Skull Orchid, `4c` Mimicron, `4d` Immortal Boneface | `uvh_4_finalchallenge` | UVH 3 |
| UVH 5 | `5a` GL, `5b` SL, `5c` MOU primordial-vault wildcards | `uvh_5_finalchallenge`; achievement `03_uvh_5`; reward cosmetic Veil | UVH 4 |
| UVH 6 | `mission_uvh_6a` (Bloomreaper) | (6a is the rank-up) | UVH 5 |
| UVH 7 | — | `stats.dlc_challenge.uvh_7` (Subjugator / Thol the Invincible) | UVH 6 |

Editor recipe for "unlock up to UVH N":
1. Require the story to be complete (`missionset_main_cityepilogue` completed, `globals.mainmissioncomplete: TRUE`), or apply the story-complete recipe first.
2. Set `globals.highest_unlocked_vault_hunter_level = N`. Insert the key if it is missing; upstream `save set` cannot do that.
3. Set `globals.vault_hunter_level` to the chosen value in 0..N.
4. Write every stat up to rank N with value `1`. Create the `stats.dlc_challenge` map for 7.
5. Mark the `missionset_main_postgame` `micro_uvh_*` missions completed.
6. In the profile, add `shared_progress.vault_hunter_level` to `unlockables.shared_progress.entries` if it is missing.
7. **Unverified:** whether the game re-derives `highest_unlocked` from the stats on load. The save-4 evidence (§7.2) is ambiguous. The stats-consistent edit is the safe choice. **Test in game once the game is closed.**

**Normal/Hard:** `state.player_difficulty` (`Easy`/`Normal`/`Hard`). **True Mode:** `state.true_mode` (bool).

### 6.2 Appearances (b)

See §5.7.
- **Equip a cosmetic:** rebuild the `state.gbxactorparts.*` string with parts from the catalogue whose unlockable ID is in the profile list. Also set `personal_vehicle` and `hover_drive` from the same unlocked lists.
- **Unlock all:** append the missing catalogue IDs to the profile lists, excluding entitlement items by default.

### 6.3 Missions (c)

- **Read:** for each set ID, the status is completed if `status == completed`, active if the key is present, and not started if it is absent. Expand to missions and objectives. Names come from `missions_catalogue.json`.
- **Complete a set:** set `status: completed`; for each mission in the set (from the NCS catalogue), `status: completed` and `ui_flags: 1`; remove `objectives`. This is what upstream `campaign.rs::mark_set_completed` does. It matches the game's own skip format except for the missing `final` endstates, which upstream omits. **Untested** whether the game needs them.
- **Complete the story:** copy the structure of save 7 (`UltimateVaultHunterSkip`).
- **Globals to set alongside missions, from saves at different points in the story:**

  | Point in the story | Globals present |
  |---|---|
  | After the prologue / beach (saves 8, 10) | `prologue_completed`, `movegrant_grapplegrabber`, `movegrant_echolocation`, `repkit_unlocked` |
  | Grasslands chain (saves 11, 12) | + `movegrant_glide` |
  | Story complete (saves 4, 5, 7, 9) | + `mainmissioncomplete`, `lockdownlifted`, `movegrant_ordonitegloves` |

  Also add the slot rewards (`unique_rewards` and `equip_slots_unlocked`), which the game grants by level, not by mission.
- **Rewind:** remove sets that come later in the chain (upstream `mark_set_reset`). Risky, because world-state and globals side effects remain.
- **Set progress inside a mission:** objective statuses (the in-mission checkpoints). Advanced only; show it read-only in v1.
- **Upstream behaviour [measured]:**
  - `missions set grasslands1` resolves to the *mission* `mission_main_grasslands1`. It only adds that mission as completed; the set keeps no status and earlier sets are untouched.
  - `missions set missionset_main_grasslands2a` completes the prerequisite sets, makes `mission_main_grasslands2` Active and rewrites `tracked_missions`.
  - `missions list` works.

### 6.4 Money, eridium, SDU, ammo, levels (d)

- **Cash and eridium:** `state.currencies.*`.
- **SDU tokens:** profile `echotokenprogresspoints`.
- **Buy all SDUs:** write all 59 `sdu_upgrades` nodes with `points_spent = cost`, and make sure tokens ≥ 3225.
- **Ammo:** fill up to the maxima from §5.4.
- **Level:** write `level` and `points = minXP` together, and set `characterprogresspoints = level − 1`. Do the same for Specialization.
- **Risk:** pools are partly derived. The game re-grants skill points per level and may recompute SDU tokens from `echo_upgrade_challenges` (a patch note mentions restoring lost SDU tokens). Prefer edits that are consistent with what the game would derive.

### 6.5 Inventory (e)

See §5.9. Item validity (serial decoding and part legality) is covered in the other research topics. On the save side:
- respect capacity;
- write contiguous slot numbers;
- keep the equipped↔backpack pairing and the `flags`/`state_flags` rules;
- bank items: `{serial, state_flags}` only.

---

## 7. Consistency, safety and the running game (f)

### 7.1 Checks inside the file

- **There is no checksum, signature or hash in the YAML.**
- **Integrity checks are external:** the zlib adler32 inside the stream, plus the length trailer. Keep both correct.
- `save_game_header.timestamp` and `state.last_played_timestamp` are not checked: bl4-edited files with old timestamps loaded fine.
- `save_game_header.guid` and `char_guid`: keep them. Duplicates across slots are tolerated.

### 7.2 Editing while the game is running is unsafe [measured evidence]

`4_20260426_203929.sav` was written by a tool at 22:39. It had cash 22,229,953, `highest_unlocked_vault_hunter_level: 6`, and UVH 2–6 stats added. The game rewrote `4.sav` at 22:42 with:
- `highest_unlocked` back to 1;
- the UVH 2–6 stats removed;
- cash back to 229,953 (exactly 22,000,000 less);
- two item serials changed;
- `lostloot.slots` 14 → 6;
- **unchanged `total_playtime` (76.419731)**.

This is consistent with the game holding the character (and the profile) in memory and overwriting the file at its next save. The upstream docs say the same: edits made after a character is loaded in the session are ignored.

During this session the game rewrote `11.sav` and `profile.sav` at the same second (16:21). The files are not locked: reading and copying worked while the game ran.

**Rules for the editor:**
- Read while the game runs, if you like. **Refuse or strongly warn on save while `Borderlands4.exe` is running.**
- Write to a temp file and rename it into place, so the write is atomic.
- Warn that Steam Cloud is enabled. It uploads the edited file on the next launch, which is fine; on a conflict, the user must pick the local copy.

### 7.3 The game accepts upstream's format [measured]

`9.sav.bak.json`, written by upstream smart backup, records `original_hash 3e80…` (equal to `9.sav.bak`) and `last_edit_hash 3dfc…`. The pre-edit `9.sav.bak` holds Rafa at **level 53 with only 826,660 XP**, inflated by the old curve mod. The edit repaired the level, and the current `9.sav` is game-written (hash `4de3…`, Sep 29) with Rafa at L32 / 1,038,492 XP. So the game loaded a file with a `78 DA` stream, an adler+len trailer and serde_yaml formatting, and kept the edited level.

This also shows that **the game never lowers a stored level to match the XP** (53 with level-30 XP stayed 53 until edited). Likewise, `8.sav.bak` was L6 with 2045 XP, and bl4 set it to L2. The backup folder's `3.sav` (78 DA) was likewise followed by game-written `3.sav` files.

### 7.4 Backups

Upstream `backup.rs`:

```rust
pub fn smart_backup(save_path: &Path) -> Result<bool, BackupError>      // creates <f>.sav.bak + <f>.sav.bak.json once
pub fn update_after_edit(save_path: &Path, metadata_path: &Path) -> Result<(), BackupError>
// metadata: {"original_hash": sha256-hex, "last_edit_hash": sha256-hex}
```

It keeps **one** backup per file, replaced only when the file changes outside the tool. The user wants a backup on every open. Instead, write timestamped copies, for example `<slot>.sav.<yyyyMMdd-HHmmss>.bak` in a `bl4editor_backups\` folder next to the saves. Back up both the character and `profile.sav`, because many features write the profile.

---

## 8. Reusable upstream code, and what is stale

| Item | Path | Status |
|---|---|---|
| AES key and decrypt | `src/bl4/src/crypto.rs`: `derive_key(&str) -> [u8;32]`, `decrypt_sav(&[u8], &str)`, `encrypt_sav(&[u8], &str)` | Decrypt is reusable. **Change encrypt** to level 6 with a 4-byte length trailer for game-identical output (it currently writes level 9 with adler+len). |
| YAML path get/set | `src/bl4/src/save/mod.rs`: `SaveFile::{from_yaml, to_yaml, get, set, set_raw}` | serde_yaml Value. **Not lossless** (§3), and `set` cannot create keys (`KeyNotFound`). |
| XP helpers | `SaveFile::{xp_for_character_level, xp_for_specialization_level, set_character_level}` | **Wrong by 439:** the TSV has no offset. `set_character_level` writes `level: 1` and relies on a kill to update the level. Use §5.5 instead. |
| Change sets | `save/changeset.rs::ChangeSet` (`add_backpack_item`, `equip_item`, `add_bank_item`, `set_all_item_levels`) | `add_backpack_item` writes `flags: 0` with `state_flags` 513, which does not match game output. `equip_item` does not touch the backpack twin. |
| State flags | `save/state_flags.rs::StateFlags` | The bit 1/2/4–7 constants are fine. The bit-9 semantics are wrong (§5.9). |
| Campaign | `save/campaign.rs` (`get_mission_status`, `plan_campaign_progress`, `apply_campaign_progress`, `plan_complete_all`, `plan_dlc_completion`, `complete_single_mission`) and `missions.rs` (embedded `share/manifest/missions/*.tsv`, `CONVERGENCE_BRANCHES` logic) | Reusable logic. The mission data is incomplete (320 of 661). Regenerate it from the NCS catalogue. |
| Entitlements | `save/entitlements.rs::detect_entitlements` | Reads `domains.local.unlockables`; still valid. |
| Fog of war | `save/fod.rs` | **Broken on current saves:** it looks for `gbx_discovery_pc.foddatas` in the character. `bl4 save X --map reveal` gives `Error: Key not found: foddatas` on both 11.sav and profile.sav. The data is now at profile `domains.local.gbx_discovery_pc_shared.foddatas`. |
| Backup | `src/bl4/src/backup.rs` | Reusable as a pattern. |
| CLI | `src/bl4-cli/src/commands/save.rs::with_save_file` | Shows the pipeline. Note that `-b/--backup` defaults to true and cannot be turned off. |

---

## 9. What did not work, and open questions

- Grepping the paks one table at a time with a shell loop is too slow (more than 10 minutes over 356 paks). Use the existing newest-copy index `game-data/ncs_newest.json` and `research/ncs_extract/base`.
- PyYAML is fine for analysis but corrupts `Off`/`On` and the `TRUE` casing on output. Do not use it to write.
- `flate2` with the `miniz_oxide` or `zlib-rs` backend cannot reproduce game bytes. Only the C zlib backend can.

**Open questions (each needs an in-game test with the game closed first):**
1. Does the game re-derive `highest_unlocked_vault_hunter_level`, SDU tokens or skill points on load?
2. What are the base backpack and bank capacities? They are in UE BP defaults, not NCS.
3. Do completed missions need `final` endstates?
4. What does `state_flags` bit 9 (512) mean?
5. What are the cash and eridium maxima?
6. Is the level-70 cap gated by an entitlement?

`Unlockable_Vehicles.Stingray` and the online patch pak (`pakchunk99` under `PersistentDownloadDir\Gearbox\Patch\bbc04541-…\`) were not scanned; `challenge_c99.bin` exists in `research/ncs_extract/bin`, so the patch touches challenges.

---

## 10. Data artifacts (copied to `research/save-fields-data/`)

| File | Content |
|---|---|
| `crypt.py` | Python decrypt and analysis (key derivation, trailer detection). |
| `emit.py` | **Reference game-style YAML emitter.** Byte-identical on all game files. Port it to Rust. |
| `pipeline_test.py` | Full pipeline `.sav → YAML → .sav` with a byte-identity check, plus a surgical cash edit. `bl4 get --money` read the edited copy back as `Cash: 999999999`. |
| `load.py` | PyYAML loader with the `!tags` constructor and path flattening. |
| `xp_table.tsv` | Minimum XP per level: Character 1..100, Specialization 1..200. |
| `sdu_nodes.tsv` | The 59 `sdu_upgrades` nodes with cost, condition and effect. |
| `progress_graphs.json` | All 134 progress graphs (skill trees, specializations, SDU, vault power) from NCS. |
| `cosmetics_catalogue.json` | 1303 unlockable IDs, each with `{group, kind, source, entry, part, name, serialindex}`. |
| `missions_catalogue.json` | 661 missions, each with `{set, type, name, region, src, n_objectives}`. |
