use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "锋锐",
    full: "锋锐魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_HIT: &[Effect] = &[Effect::Slow {
    target: Target::EnemyRandomActiveItems(1),
    seconds: 4.0,
}];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnHit,
    effects: ON_HIT,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
