Domain Model Rules

The domain model represents Borderlands 4 concepts, not raw bytes.

Layering

Prefer:

File Format
    ↓
Serialization
    ↓
Domain Model
    ↓
Operations
    ↓
UI


The UI should not manipulate binary offsets.

Domain Objects

Potential concepts include:

Character.

Inventory.

Item.

Weapon.

Equipment.

Mission.

Progression.

Currency.

Skill.

Challenge.

Collectible.

Profile/account data.

Only create a concept when supported by actual requirements or evidence.

Strong Types

Use strong types where they prevent meaningful errors.

Do not create abstractions solely for theoretical future requirements.

Unknown Values

Unknown enum values must not automatically cause data loss.

Prefer representations that can retain the original value.

Example conceptually:

KnownEnum(value)
UnknownEnum(rawValue)


where appropriate.

Mutation

Prefer explicit operations:

AddItem
RemoveItem
CloneItem
ModifyItem
SetCurrency
SetCharacterLevel
SetMissionState


This provides natural places for:

Validation.

Logging.

Undo.

Testing.

Invariants

Domain objects should expose meaningful invariants where known.

Examples:

Valid references.

Valid ranges.

Required relationships.

Unique identifiers.

Do not enforce speculative constraints.

If the game accepts something unusual, the editor should not reject it merely because it looks strange.