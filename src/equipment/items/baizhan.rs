use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "百战",
    full: "百战魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::MaxHp(180), Effect::Heal(180)];
const ON_HEAL: &[Effect] = &[Effect::AttackBonus {
    target: Target::OwnAllWeapons,
    amount: 16,
    max_stacks: Some(10),
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::OnHeal,
        effects: ON_HEAL,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
