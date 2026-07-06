use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "玄武",
    full: "玄武之力",
    kind: Kind::Potion,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_NORMAL_HIT: &[Effect] = &[
    Effect::Uses(1),
    Effect::MaxHp(200),
    Effect::Heal(200),
    Effect::AttackBonus {
        target: Target::OwnAllWeapons,
        amount: 40,
        max_stacks: None,
    },
    Effect::Accelerate {
        target: Target::OwnAllWeapons,
        seconds: 3.0,
    },
];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnNormalHit,
    effects: ON_NORMAL_HIT,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
