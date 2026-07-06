use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "寒冰核",
    full: "寒冰核护符",
    kind: Kind::Charm,
    w: 2,
    h: 2,
    attack: 0.0,
    interval: 6.0,
};

const ACTIVE: &[Effect] = &[Effect::Freeze {
    target: Target::EnemyRandomActiveItems(1),
    seconds: 3.5,
}];
const ON_NORMAL_HIT: &[Effect] = &[Effect::Slow {
    target: Target::EnemyRandomActiveItems(1),
    seconds: 1.0,
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::ActiveUse,
        effects: ACTIVE,
    },
    EffectSpec {
        trigger: Trigger::OnNormalHit,
        effects: ON_NORMAL_HIT,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
