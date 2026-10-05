Serialization Rules

Serialization is safety-critical.

Parser ≠ Serializer

A successful parser does not prove the serializer is correct.

Test both independently.

Round Trip

For every stable structure:

bytes
→ parse
→ model
→ serialize
→ bytes/model


Validate the expected invariants.

Byte Preservation

Byte-for-byte equality is desirable when the format permits it.

But do not force byte equality when serialization legitimately normalizes:

Ordering.

Padding.

Compression.

Metadata.

Equivalent encodings.

Document normalization behavior.

Failure Safety

If serialization fails:

Do not overwrite the original.

Report the failure.

Preserve the in-memory data if possible.

Leave recovery options.

Temporary Files

Prefer:

original
↓
write temporary output
↓
validate
↓
atomic replacement


rather than writing directly into the original file.

Compression

If compression is involved:

Validate decompression.

Validate recompression.

Test empty data.

Test small data.

Test large data.

Test malformed compressed data.

Do not assume standard compression merely because the data resembles it.

Checksums

If a checksum exists:

Identify algorithm.

Identify covered region.

Identify byte order.

Identify storage representation.

Validate across multiple files.

Never hard-code a checksum algorithm based on one matching sample.

Serialization Tests

At minimum test:

Empty structures.

Typical structures.

Large structures.

Unknown fields.

Optional fields.

Multiple versions.

Malformed data.

Round trips.