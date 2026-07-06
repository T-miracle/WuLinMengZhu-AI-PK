use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "缚命环",
    full: "缚命环",
    kind: Kind::Charm,
    w: 2,
    h: 2,
    attack: 0.0,
    interval: 5.5,
};

const ACTIVE: &[Effect] = &[Effect::Heal(90)];
const PASSIVE_AFTER_ACTIVE: &[Effect] = &[Effect::Heal(40)];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::ActiveUse,
        effects: ACTIVE,
    },
    EffectSpec {
        trigger: Trigger::Passive,
        effects: PASSIVE_AFTER_ACTIVE,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
