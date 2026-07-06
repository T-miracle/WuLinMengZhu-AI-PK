use crate::equipment::{
    DamageAmount, Effect, EffectSpec, EquipmentSpec, StarProfile, Target, Trigger,
};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "龙弧",
    full: "龙弧近战武器",
    kind: Kind::Weapon,
    w: 2,
    h: 2,
    attack: 75.0,
    interval: 6.0,
};

const CHARGED_ATTACK: &[Effect] = &[
    Effect::ConsumeSword(28),
    Effect::Damage(DamageAmount::WeaponAttackMultiplier(3.7)),
    Effect::EnemySword(-16),
    Effect::Freeze {
        target: Target::EnemyRandomActiveItems(1),
        seconds: 2.0,
    },
];
const NORMAL_ATTACK: &[Effect] = &[
    Effect::Damage(DamageAmount::WeaponAttack),
    Effect::Sword(16),
];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::ChargedAttack,
        effects: CHARGED_ATTACK,
    },
    EffectSpec {
        trigger: Trigger::NormalAttack,
        effects: NORMAL_ATTACK,
    },
];

pub const STAR_PROFILES: [StarProfile; 4] = [
    StarProfile {
        star: 1,
        attack: 30.0,
        interval: 7.5,
        normal_sword: 10,
        freeze_attack_bonus: None,
        charged_enemy_sword: Some(10),
    },
    StarProfile {
        star: 2,
        attack: 45.0,
        interval: 7.0,
        normal_sword: 12,
        freeze_attack_bonus: None,
        charged_enemy_sword: Some(12),
    },
    StarProfile {
        star: 3,
        attack: 60.0,
        interval: 6.5,
        normal_sword: 14,
        freeze_attack_bonus: None,
        charged_enemy_sword: Some(14),
    },
    StarProfile {
        star: 4,
        attack: 75.0,
        interval: 6.0,
        normal_sword: 16,
        freeze_attack_bonus: None,
        charged_enemy_sword: Some(16),
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
