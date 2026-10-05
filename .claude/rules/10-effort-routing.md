Effort Routing

The purpose of this file is to prevent excessive reasoning/token usage.

Rule

Choose effort based on:

uncertainty × consequence × complexity


not repository size.

A task in a complicated repository can still be LOW effort.

LOW

Use LOW when the intended change is already obvious.

Examples:

Rename.

Formatting.

Comment.

Documentation.

Simple UI label.

Existing helper usage.

Existing-pattern implementation.

Obvious compiler error.

Straightforward null handling.

Running tests.

Small repetitive change.

Behavior

Do not perform broad repository exploration.

Inspect only enough surrounding code to avoid making a mistake.

Implement immediately.

MEDIUM

Use MEDIUM when the task requires understanding but the underlying behavior is known.

Examples:

New editor feature.

New model property.

New command.

New validation.

Normal bug.

New UI component.

New test suite.

Known serialization structure.

Refactor with clear boundaries.

Behavior

Inspect:

Relevant implementation.

Related model.

Existing tests.

Similar functionality.

Then implement.

HIGH

Use HIGH when failure could indicate an architectural or data-integrity problem.

Examples:

Save corruption.

Serialization mismatch.

Version compatibility.

Cross-layer bugs.

Large refactor.

Unknown interaction between systems.

Persistent game rejection.

Data unexpectedly disappearing.

Behavior

Reproduce first.

Then isolate.

Then form hypotheses.

Then modify code.

MAX

MAX is reserved for fundamental uncertainty.

Use it for:

Unknown binary structure.

Unknown compression.

Unknown encryption.

Unknown checksums.

Unknown serialization.

Contradictory save-file evidence.

Game rejecting otherwise apparently valid files.

Fundamental format architecture.

Reverse engineering requiring controlled experiments.

MAX does NOT mean

"Read the entire repository."

It means:

"Spend significant reasoning effort on the actual unknown."

Escalation

Start low.

Escalate only when evidence requires it.

Example:

LOW
↓
Simple implementation fails
↓
MEDIUM
↓
Root cause unclear
↓
HIGH
↓
Evidence reveals unknown format behavior
↓
MAX


Do not start at MAX because the task contains words like "binary", "serialization", or "save".

Stop Conditions

Stop investigating when:

The evidence is sufficient.

The implementation is validated.

Additional investigation is unlikely to change the conclusion.

Do not continue researching merely because more information exists.

Token Discipline

Avoid:

Re-reading unchanged files.

Reading unrelated directories.

Re-explaining obvious code.

Repeated searches for the same answer.

Speculative architecture.

Premature optimization.

Broad refactoring.

Rebuilding known functionality.

Prefer:

Targeted searches.

Focused context.

Small diffs.

Existing patterns.

Controlled experiments.

Incremental validation.