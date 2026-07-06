use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "嗜血",
    full: "嗜血魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_CRITICAL_HIT: &[Effect] = &[Effect::LifeStealPct {
    target: Target::OwnAllWeapons,
    pct: 6.0,
    max_stacks: Some(20),
}];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnCriticalHit,
    effects: ON_CRITICAL_HIT,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
