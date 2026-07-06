use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "续命术",
    full: "续命术魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_FREEZE: &[Effect] = &[Effect::Heal(70)];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnFreeze,
    effects: ON_FREEZE,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
