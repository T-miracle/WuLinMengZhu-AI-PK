use crate::equipment::{
    DamageAmount, Effect, EffectSpec, EquipmentSpec, StarProfile, Target, Trigger,
};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "冰魄",
    full: "冰魄近战武器",
    kind: Kind::Weapon,
    w: 1,
    h: 3,
    attack: 110.0,
    interval: 7.0,
};

const NORMAL_ATTACK: &[Effect] = &[
    Effect::Damage(DamageAmount::WeaponAttack),
    Effect::Sword(16),
    Effect::Freeze {
        target: Target::EnemyRandomActiveItems(2),
        seconds: 2.0,
    },
];
const ON_FREEZE: &[Effect] = &[Effect::AttackBonus {
    target: Target::OwnAllWeapons,
    amount: 35,
    max_stacks: Some(7),
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::NormalAttack,
        effects: NORMAL_ATTACK,
    },
    EffectSpec {
        trigger: Trigger::OnFreeze,
        effects: ON_FREEZE,
    },
];

pub const STAR_PROFILES: [StarProfile; 4] = [
    StarProfile {
        star: 1,
        attack: 60.0,
        interval: 7.0,
        normal_sword: 16,
        freeze_attack_bonus: Some(20),
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 2,
        attack: 110.0,
        interval: 6.5,
        normal_sword: 16,
        freeze_attack_bonus: Some(35),
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 3,
        attack: 160.0,
        interval: 6.0,
        normal_sword: 16,
        freeze_attack_bonus: Some(50),
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 4,
        attack: 210.0,
        interval: 5.5,
        normal_sword: 16,
        freeze_attack_bonus: Some(65),
        charged_enemy_sword: None,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
