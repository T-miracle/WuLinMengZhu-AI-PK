use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "穿颅",
    full: "穿颅魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[
    Effect::Accelerate {
        target: Target::OwnAllWeapons,
        seconds: 6.0,
    },
    Effect::Charge {
        target: Target::OwnAdjacentActiveItems,
        seconds: 3.0,
    },
];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::BattleStart,
    effects: START,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
