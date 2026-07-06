use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "天响",
    full: "天响魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::Sword(22)];
const ON_PARRY: &[Effect] = &[Effect::Charge {
    target: Target::OwnAllActiveItems,
    seconds: 1.5,
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
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
