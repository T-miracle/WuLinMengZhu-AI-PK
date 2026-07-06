use crate::equipment::{Effect, EffectSpec, EquipmentSpec, Target, Trigger};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "白虎之力",
    full: "白虎之力",
    kind: Kind::Potion,
    w: 1,
    h: 1,
    attack: 0.0,
    interval: 0.0,
};

const ON_CHARGED_HIT: &[Effect] = &[
    Effect::Uses(1),
    Effect::Freeze {
        target: Target::EnemyAllActiveItems,
        seconds: 3.0,
    },
    Effect::AttackBonus {
        target: Target::OwnAllWeapons,
        amount: 100,
        max_stacks: None,
    },
];

pub const PROPERTIES: &[EffectSpec] = &[EffectSpec {
    trigger: Trigger::OnHit,
    effects: ON_CHARGED_HIT,
}];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
