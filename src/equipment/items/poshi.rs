use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "破势",
    full: "破势魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_HIT: &[Effect] = &[Effect::EnemySword(-12)];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnHit,
    effects: ON_HIT,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
