Save Format Rules

The save format is a first-class domain.

Treat it as an external protocol.

Parsing Principles

Parsers must:

Validate input.

Avoid unsafe allocations.

Detect malformed structures.

Preserve unknown data where possible.

Report useful errors.

Avoid silently recovering from corruption.

Do not silently reinterpret malformed data.

Unknown Fields

Unknown fields should be represented whenever possible.

Preferred approaches include:

Known typed field
+
Unknown/raw representation


or:

Raw serialized structure
+
Typed projection


Choose the simplest architecture that preserves information.

Versioning

Do not scatter arbitrary version checks throughout business logic.

Prefer version-aware format boundaries.

Conceptually:

File
 ↓
Version detection
 ↓
Version-specific parser
 ↓
Stable domain representation
 ↓
Version-specific serializer


Do not force incompatible versions into one parser merely to reduce code.

IDs

Determine whether each ID is:

Unique.

Persistent.

Generated.

Save-specific.

Character-specific.

An index.

A reference.

Do not regenerate IDs casually.

References

If structures reference one another:

Maintain referential integrity.

Validate references.

Test deletion.

Test cloning.

Test duplication.

Test save/load round trips.

Unknown Data Policy

If the editor does not understand a structure:

Preserve it.

Do not normalize it.

Do not rewrite it unnecessarily.

Document it as unknown.

Expose it only through advanced UI if useful.

Data Loss

Any operation that may discard information must be explicit.

Never silently convert:

unknown → absent


unless that behavior is proven safe.