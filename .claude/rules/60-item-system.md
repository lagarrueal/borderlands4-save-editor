Item System Rules

Items are expected to be one of the most complex systems in the editor.

Do not oversimplify the representation.

Potential concepts may include:

Item balance.

Manufacturer.

Parts.

Part parameters.

Level.

Rarity.

Prefix.

Suffix.

Affixes.

Elements.

Scaling.

Unique identifiers.

Generation metadata.

Inventory state.

Equipped state.

These are possibilities, not assumptions.

Item Reverse Engineering

When investigating items:

Change exactly one item characteristic at a time.

Examples:

Item A
Item B


where only one of the following differs:

Level.

Manufacturer.

Part.

Element.

Rarity.

Prefix.

Suffix.

Quantity.

Then compare serialized data.

Item Cloning

Do not assume every field should be copied.

Classify fields as:

COPY
REGENERATE
RESET
UNIQUE
RECALCULATE
UNKNOWN


before implementing sophisticated cloning.

Item IDs

Never assume duplicated items may share IDs.

Determine the game's actual identifier semantics first.

Item Validation

Do not reject unusual items simply because they are unusual.

Distinguish:

Invalid according to format


from:

Unusual but potentially valid


The editor should not unnecessarily restrict experimentation.