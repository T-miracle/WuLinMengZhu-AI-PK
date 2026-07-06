use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "神将甲",
    full: "神将甲护甲",
    kind: Kind::Armor,
    w: 2,
    h: 3,
    attack: 0.0,
    interval: 0.0,
};

const START: &[Effect] = &[Effect::Armor(300), Effect::DamageReductionPct(5.0)];
const ON_DISABLED: &[Effect] = &[Effect::Armor(15), Effect::Heal(15)];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::OnDisabled,
        effects: ON_DISABLED,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
