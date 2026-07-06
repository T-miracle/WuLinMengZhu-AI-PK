use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "铸盾",
    full: "铸盾魂玉",
    kind: Kind::Soul,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_HEAL: &[Effect] = &[Effect::Armor(45)];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnHeal,
    effects: ON_HEAL,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
