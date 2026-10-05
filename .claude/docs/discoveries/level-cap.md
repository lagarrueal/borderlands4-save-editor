# Level cap and XP curve

Status: CONFIRMED (data) / LIKELY (XP above 60 not yet seen in a game-written save)

Observation:
- xp_progression (NCS) after the level-cap patch: oak2_characterxp_progression levelcap 70,
  two GbxExperienceFunction_Exponential segments (maxlevel 60, 100), both multiplier 60, power 2.8, offset 7.33.
- oak2_specializationxp_progression levelcap 701, segments maxlevel 50 / 100, multiplier 80, power 2.8, offset 7.33.
- Game saves (2026-10-05 copies): every character at L60 or below stores XP within
  min_xp(L)..min_xp(L+1); 6.sav spec L110 too (so the last segment continues past maxlevel 100).
  6.sav holds 28.5M character XP at L60 (capped before the patch), which maps to L70 under the new cap.

Interpretation: min_xp(L) = floor(m x (L^2.8 + 7.33)) is unchanged; only the caps moved.
Editor: MAX_CHAR_LEVEL 70, MAX_SPEC_LEVEL 701, item MAX_LEVEL = character cap.
tests/level_cap.rs fails if a game patch changes the caps or the curve (DB key "xp").

Before this fix (v1.0.0): saves with a character above 60 failed check_invariants (save refused),
the level box clamped to 60, and items above 60 were flagged as invalid.

Open questions: confirm a game-written L61-70 character's XP once the game re-levels 6.sav;
the item level cap is assumed equal to the character cap (INFERRED).
