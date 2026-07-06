use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "通达",
    full: "通达魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::Accelerate {
    target: Target::OwnAllActiveItems,
    seconds: 2.0,
}];
const PASSIVE: &[Effect] = &[Effect::AttackBonus {
    target: Target::OwnAdjacentWeapons,
    amount: 18,
    max_stacks: Some(20),
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::Passive,
        effects: PASSIVE,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
