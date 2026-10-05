# Changelog

## 1.0.1

- Level cap 70 (game update): characters above level 60 could not be saved,
  and the level box clamped to 60. Character, specialization and item level
  caps now follow the game data (character 70, specialization 701).
- Heavy weapons: cooldown, magazine, DPS, sell value and name prefixes
  ("Junk-Drunk Sidewinder", "Eager Sprezzatura"), matching in-game cards.
- Ripper weapons: magazine, reload and DPS including the charge-up time.
- Shot cost line ("2/Shot") on weapons that use more than one round per shot.
- Weapon names no longer change between runs when two licensed-part tags match.

## 1.0.0

First release.

- Item editor for backpack, equipped gear, bank and Lost Loot (weapons, heavy
  weapons, shields, ordnance, repkits, enhancements, class mods): every part in
  a searchable dropdown limited to its slot, add/remove parts, level,
  favorite/junk, serial codes, duplicate, move, delete, build new items.
- Item card with full in-game name (stat and licensed-part prefixes), rarity (incl. pearlescent), manufacturer logo, element,
  firmware, effect text with real numbers and red flavour text.
- Weapon stats computed from the game data and checked against in-game cards:
  damage, accuracy, fire rate (incl. burst weapons), magazine, reload time,
  DPS, elemental DPS and chance, splash radius, crit, approximate sell value.
  Base stats for shields, ordnance and repkits.
- Validity check of every item against the game data; confirmation before
  saving items the game would reject; mod-only parts recognised.
- Scale an item, the backpack or the bank to the character level.
- Character level/XP, specialization, difficulty, True Mode, spawn checkpoint.
- Cash, eridium, SDU tokens, Vault Card tokens, ammo.
- UVH 1-7 unlock, SDU upgrades, equipment slots.
- Missions: complete story (game's own skip records), sets, missions,
  objective states.
- Cosmetics: equip and unlock.
- Automatic backup of the character and profile on open; saving blocked while
  the game runs; byte-identical writes; capacity checks; undo/redo.
