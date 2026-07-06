use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "蜃景苞",
    full: "蜃景苞护符",
    kind: Kind::Charm,
    w: 2,
    h: 2,
    attack: 0.0,
    interval: 7.5,
};

const ACTIVE: &[Effect] = &[Effect::Freeze {
    target: Target::EnemyRandomActiveItems(1),
    seconds: 1.1,
}];
const ON_FREEZE: &[Effect] = &[Effect::Charge {
    target: Target::OwnRandomActiveItems(1),
    seconds: 1.5,
}];

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
