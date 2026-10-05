# Item serials (@Ug...)

Status: CONFIRMED

Observation:
- `@Ug` + custom base85; the bitstream is read LSB-first from the raw bytes.
- Tokens: Sep `00`, Soft `01`, VarInt `100`, VarBit `110`, Part `101`, String `111`.
- Part value forms: none `{n}` (own category), single `{cat:n}`, list `{cat:[..]}` written `0 01 01 (100 v)* 00`.
- Header: `cat, 0, 1, level | 2, seed | |`.
- Shared part pools: weapon 1, classmod 234, armor 237, repkit 243, heavy 244, grenade 245, shield 246, enhancement 247, energy 248.

Evidence: 673 distinct serials from the test saves decode and re-encode to the identical string (tests/serial_corpus.rs).

Validation rules (measured on 654 game items, no false positives):
- ERROR: unknown category/part, part from a pool the item kind cannot use, level outside 1..60 (the game moves such items to `unknown_items`).
- WARNING: pool slot not allowed, more than one part in a single-instance slot, comp/exclusion/dependency tags, mod-only parts.

Relevant code: crates/bl4core/src/serial.rs, item.rs, tests/validator_fuzz.rs.

Open questions: the game's exact behaviour on WARNING-class items is only partly tested in game.
