use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "振魄",
    full: "振魄魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::Sword(20)];
const ON_CHARGED_HIT: &[Effect] = &[
    Effect::Sword(20),
    Effect::Freeze {
        target: Target::EnemyRandomActiveItems(1),
        seconds: 1.0,
    },
];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::OnChargedHit,
        effects: ON_CHARGED_HIT,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
