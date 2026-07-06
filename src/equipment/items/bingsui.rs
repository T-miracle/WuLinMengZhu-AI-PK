use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "冰髓",
    full: "冰髓护符",
    kind: Kind::Charm,
    w: 2,
    h: 2,
    attack: 0.0,
    interval: 8.0,
};

const ACTIVE: &[Effect] = &[Effect::Freeze {
    target: Target::EnemyRandomActiveItems(2),
    seconds: 1.5,
}];
const ON_FREEZE: &[Effect] = &[Effect::Sword(10)];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::ActiveUse,
        effects: ACTIVE,
    },
    EffectSpec {
        trigger: Trigger::OnFreeze,
        effects: ON_FREEZE,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
