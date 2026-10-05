BL4 Save Editor {version}
========================

An offline, standalone save editor for Borderlands 4.
Source and issues: https://github.com/lagarrueal/borderlands4-save-editor

INSTALL
-------
Nothing to install. Unzip anywhere and run BL4SaveEditor.exe (Windows 10/11, 64-bit).

HOW TO USE
----------
1. Quit Borderlands 4 completely. The editor refuses to save while the game
   runs, because the game keeps your character in memory and would overwrite
   the file.
2. Turn off Steam Cloud for Borderlands 4 while you edit (Steam > Library >
   Borderlands 4 > Properties > General), or pick the local files when Steam
   asks after editing.
3. Run BL4SaveEditor.exe. Your saves are found automatically in
   Documents\My Games\Borderlands 4\Saved\SaveGames\<steamid>\Profiles\client
4. Click a character on the left, make your changes, press Save (Ctrl+S).
   Undo/Redo: Ctrl+Z / Ctrl+Y.

BACKUPS
-------
Each time you open a character, the character file and profile.sav are copied
to the "bl4editor_backups" folder next to your saves, with the date and time
in the name. To restore one, copy it back over N.sav / profile.sav and remove
the "_date-time" part of its name.

WHAT YOU CAN EDIT
-----------------
- Items in the backpack, equipped slots, bank and Lost Loot: parts (searchable
  lists), level, favorite/junk, copy/paste item codes, duplicate, move between
  backpack and bank, delete, create new items. Scale one item or the whole
  backpack/bank to your character level.
- Item card: name, rarity, manufacturer, element, firmware, red text, effects
  and stats (damage, accuracy, fire rate, magazine, reload, DPS, element,
  splash radius, crit) computed from the
  game's own data.
- Validity check: items the game would reject are marked with a red cross and
  you are asked before saving them. Orange marks are items that cannot drop
  normally but load fine.
- Character: name, level/XP, specialization level, difficulty, True Mode,
  spawn checkpoint.
- Cash, eridium, SDU tokens, Vault Card tokens, ammo.
- UVH 1-7 unlock and active UVH level; SDU upgrades; equipment slots.
- Missions: complete the story, sets, missions; set objective states.
- Appearance: equipped cosmetics; unlock cosmetics.
- Raw YAML view for advanced users.

GAME UPDATES
------------
The game data inside the exe matches the game version it was built for. New
items from later patches show as unknown parts until the editor is updated;
they are never removed from your save by the editor.

NOTES
-----
- Golden keys are stored by SHiFT on Gearbox's servers and cannot be edited.
- Keep backups. Edit at your own risk.
- Not affiliated with Gearbox Software or 2K.
