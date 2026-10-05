Borderlands 4 Save Editor — Agent Instructions
Mission

Build a reliable, extensible Borderlands 4 save editor with feature parity inspired by mature Gibbed-style Borderlands save editors, plus additional modern functionality.

The project's highest priorities are:

Save-data safety.

Correct parsing and serialization.

Preservation of unknown data.

Evidence-based reverse engineering.

Testability.

Maintainability.

Feature completeness.

UI polish.

Never trade save-data integrity for convenience.

Instruction Loading

Before working, determine which specialized rules apply.

Relevant rules live in:

rules/10-effort-routing.md

rules/20-reverse-engineering.md

rules/30-save-format.md

rules/40-serialization.md

rules/50-domain-model.md

rules/60-item-system.md

rules/70-testing.md

rules/80-ui.md

rules/90-git.md

Read only the rules relevant to the current task.

Do NOT read every rule file for every task.

Effort Policy

Use the lowest reasoning effort that can reliably solve the task.

LOW

Use for:

Mechanical edits.

Formatting.

Comments.

Simple renames.

Obvious fixes.

Existing-pattern changes.

Straightforward UI changes.

Running tests.

MEDIUM

Use for:

Normal feature work.

New code following established patterns.

Domain-model changes with known schemas.

Normal bug fixes.

Validation.

Tests.

Editor functionality.

HIGH

Use for:

Complex bugs.

Serialization problems.

Version compatibility.

Cross-subsystem changes.

Data-preservation problems.

Architectural changes.

MAX

Use only for:

Unknown save structures.

Reverse engineering.

Unknown serialization.

Compression/encryption/checksum discovery.

Game-rejection problems.

Fundamental serialization architecture.

Situations where incorrect conclusions could corrupt saves.

Do not use maximum effort merely because the repository is large.

General Development Rules

Inspect before editing.

Find existing patterns before inventing new ones.

Keep changes focused.

Do not perform unrelated refactors.

Test changes whenever practical.

Never claim tests were run if they were not.

Do not invent unknown save-format information.

Preserve unknown data whenever possible.

Never knowingly overwrite or corrupt the user's original save.

Prefer reversible operations.

Prefer evidence over assumptions.

Fix root causes rather than symptoms.

Reverse Engineering

For unknown save structures, follow the reverse-engineering workflow in:

rules/20-reverse-engineering.md

Do not guess a field's meaning merely because its value looks plausible.

Separate:

OBSERVED

DOCUMENTED

INFERRED

HYPOTHESIS

UNKNOWN

CONFIRMED

Record important discoveries under:

docs/discoveries/

Serialization

Serialization is safety-critical.

Follow:

rules/40-serialization.md

A parser succeeding does not prove the writer is correct.

For important format changes, test:

original
→ parse
→ model
→ serialize
→ parse


Where possible, also test the resulting save in the actual game.

Unknown Data

Unknown data must be treated as valuable.

Do not discard information merely because the editor does not understand it.

Prefer round-tripping unknown fields or retaining their raw representation.

If preservation is impossible, make that limitation explicit.

Debugging

When something fails, identify the earliest incorrect stage:

Input
→ Parse
→ Domain model
→ Edit
→ Validation
→ Serialize
→ Output
→ Game


Do not patch downstream symptoms before determining where the first incorrect state appears.

Gibbed Compatibility

Gibbed Borderlands editors are a feature and UX reference.

Do not assume their internal data model applies to Borderlands 4.

Treat older-game behavior as a hypothesis until confirmed against Borderlands 4 evidence.

User Interaction

Do not ask the user for information that can be obtained from:

The repository.

Existing documentation.

Existing tests.

Existing save samples.

Existing code.

Controlled experiments.

Ask only when the answer materially affects implementation and cannot reasonably be determined otherwise.

Completion Standard

A task is complete when:

The requested behavior exists.

Relevant code compiles.

Relevant tests pass where available.

Save-data safety has been considered.

No unnecessary unrelated changes were introduced.

Important discoveries have been documented.

For reverse engineering, completion additionally requires evidence and validation.

Final Principle

Move quickly when the problem is known.

Slow down when the problem is uncertain.

Never use expensive reasoning to compensate for lack of discipline.

Never use a guess where an experiment can provide evidence.