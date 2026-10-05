Git Rules

Keep history useful.

Commits

Prefer focused commits:

Add save header parser


rather than:

Rewrite everything

Avoid Unrelated Changes

Do not combine:

Feature work.

Formatting.

Dependency upgrades.

Large refactors.

Renames.

unless they are actually required.

Reverse Engineering Commits

When possible, separate:

Experiment/documentation


from:

Implementation


This makes the history useful when format assumptions later change.

Before Commit

Check:

Diff.

Tests.

Generated files.

Accidental formatting.

Debug code.

Temporary files.

Sensitive save data.

Never commit real user saves.

Save Samples

Test saves may contain personally meaningful information.

Keep them in appropriate locations and do not commit user-provided saves unless deliberately intended.