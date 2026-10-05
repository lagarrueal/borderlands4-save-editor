# Save container (.sav)

Status: CONFIRMED

Observation:
- File = AES-256-ECB ciphertext. Key = fixed base key with its first 8 bytes XORed with the Steam ID (u64 LE).
- Plaintext = PKCS7( zlib(level 6) stream + u32 LE uncompressed length ).
- Payload is a YAML subset: `key: ` keeps a trailing space for empty values, `!tags` on nodes, no final newline.

Evidence:
- 13/13 game-written saves decrypt -> parse -> emit -> re-encrypt to identical bytes (tests/save_corpus.rs).
- Only the C zlib backend (flate2 feature "zlib") reproduces the game's deflate stream byte for byte; miniz differs.

Interpretation: lossless round-trip is possible, so edits only change what they touch.

Confidence: high (multiple characters, profiles, versions).

Relevant code: crates/bl4core/src/crypto.rs, yaml.rs, save.rs (write = temp file + verify + rename).

Open questions: none for reading/writing. Tool-written YAML is not asserted byte-exact against the game's own emitter.
