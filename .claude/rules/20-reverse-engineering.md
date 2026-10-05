Reverse Engineering Rules

Reverse engineering is different from ordinary programming.

The goal is to determine what the data actually represents.

Never encode an assumption as fact without evidence.

Evidence Hierarchy

Prefer evidence in this order:

Controlled Borderlands 4 save experiments.

Multiple independent save samples.

Reproducible game behavior.

Existing project tests.

Reliable technical documentation.

Existing implementations.

Community research.

Intuition.

Lower-ranked evidence may generate hypotheses but should not override stronger contradictory evidence.

Evidence Labels

Use:

OBSERVED

Directly measured.

Example:

Bytes 0x20–0x23 changed when character level changed.

INFERRED

Strong conclusion from observations.

HYPOTHESIS

Plausible but unconfirmed.

CONFIRMED

Validated by multiple independent experiments.

UNKNOWN

No reliable interpretation.

Never Guess

Never infer a field's meaning solely from:

Its name.

Its size.

Its numerical value.

Similarity to another Borderlands game.

A forum post.

A decompiler's variable name.

These can create hypotheses.

They are not proof.

Controlled Experiments

Prefer one-variable experiments.

Example:

Save A:
Character level = 10

Save B:
Character level = 11

Everything else unchanged.


Compare A and B.

Then repeat.

Experiment Record

Store important experiments under:

docs/experiments/

Use:

# Experiment: <name>

Date:

Question:

Initial hypothesis:

Input save:

Controlled change:

Observed difference:

Interpretation:

Confidence:

Follow-up:

Discovery Record

Important discoveries belong in:

docs/discoveries/

Use:

# <Discovery>

Status: UNKNOWN | HYPOTHESIS | LIKELY | CONFIRMED

Observation:

Evidence:

Interpretation:

Confidence:

Affected versions:

Related structures:

Relevant code:

Open questions:

Multiple Samples

Never declare a field understood from one save unless there is overwhelming external evidence.

Prefer validation across:

Different characters.

Different levels.

Different inventories.

Different progression.

Different game states.

Different save versions.

Boundary Testing

For numeric fields, investigate:

Zero.

One.

Typical value.

Large value.

Maximum plausible value.

Negative value where applicable.

Empty/null representation.

Do not blindly write extreme values into real saves.

Use copies.

Contradictions

When evidence conflicts:

DO NOT choose the convenient interpretation.

Instead:

Record the contradiction.

Identify differing conditions.

Generate competing hypotheses.

Design a distinguishing experiment.

Test.

Update documentation.

Reverse Engineering Output

A successful reverse-engineering task should ideally produce:

Evidence
+
Documented interpretation
+
Implementation
+
Test


Do not keep critical discoveries only inside source code comments.