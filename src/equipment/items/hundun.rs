use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "混沌",
    full: "混沌魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::Sword(20)];
const NORMAL_ATTACK: &[Effect] = &[Effect::Sword(8)];
const ON_PARRY: &[Effect] = &[Effect::Accelerate {
    target: Target::OwnAllWeapons,
    seconds: 4.0,
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::NormalAttack,
        effects: NORMAL_ATTACK,
    },
    EffectSpec {
        trigger: Trigger::OnParry,
        effects: ON_PARRY,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
