Testing Rules

Testing priorities:

Data integrity.

Parser correctness.

Serializer correctness.

Round-trip preservation.

Domain correctness.

Editing operations.

Validation.

UI.

Cosmetics.

Test Saves

Maintain representative saves in testdata/ (gitignored):

testdata/saves/   copies of game-written saves (never the live save folder)

Tests find the Steam ID via BL4_STEAM_ID or known_steam_ids().


Use copies.

Never experiment against the user's original saves.

Golden Saves

Maintain representative examples for:

New character.

Mid progression.

Advanced progression.

Empty inventory.

Large inventory.

Diverse items.

Missions.

Currency.

Unusual data.

Different format versions.

Round-Trip Tests

Test:

Load
→ Save
→ Load


and compare the semantic representation.

Where possible also test:

Load
→ Save
→ byte comparison


when byte preservation is expected.

Mutation Tests

For every important editor operation:

Load
→ mutate
→ validate
→ save
→ reload
→ verify mutation

Negative Tests

Test malformed input:

Truncated files.

Invalid lengths.

Invalid values.

Corrupted compression.

Missing fields.

Unknown fields.

Unsupported versions.

The application should fail safely.

Regression Tests

Every significant discovered bug should become a regression test where practical.

Especially:

Corruption bugs.

Data-loss bugs.

Incorrect parsing.

Incorrect serialization.

Version-specific bugs.

Real Game Validation

When possible:

Editor
→ generated save
→ actual game
→ successful load


A parser test cannot prove game compatibility.

Do not claim game validation without actually performing it.