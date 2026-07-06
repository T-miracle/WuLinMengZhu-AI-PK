use crate::equipment::{
    DamageAmount, Effect, EffectSpec, EquipmentSpec, StarProfile, Target, Trigger,
};
use crate::{Def, Kind};

pub const DEF: Def = Def {
    short: "烛神令",
    full: "烛神令",
    kind: Kind::Weapon,
    w: 1,
    h: 3,
    attack: 125.0,
    interval: 6.5,
};

const START: &[Effect] = &[Effect::Sword(20)];
const CHARGED_ATTACK: &[Effect] = &[
    Effect::ConsumeSword(24),
    Effect::Damage(DamageAmount::WeaponAttackMultiplier(2.25)),
    Effect::Heal(4),
];
const NORMAL_ATTACK: &[Effect] = &[
    Effect::Damage(DamageAmount::WeaponAttack),
    Effect::Sword(14),
    Effect::Heal(4),
];
const ON_HEAL: &[Effect] = &[Effect::Charge {
    target: Target::ThisWeapon,
    seconds: 1.0,
}];

pub const PROPERTIES: &[EffectSpec] = &[
    EffectSpec {
        trigger: Trigger::BattleStart,
        effects: START,
    },
    EffectSpec {
        trigger: Trigger::ChargedAttack,
        effects: CHARGED_ATTACK,
    },
    EffectSpec {
        trigger: Trigger::NormalAttack,
        effects: NORMAL_ATTACK,
    },
    EffectSpec {
        trigger: Trigger::OnHeal,
        effects: ON_HEAL,
    },
];

pub const STAR_PROFILES: [StarProfile; 4] = [
    StarProfile {
        star: 1,
        attack: 55.0,
        interval: 7.5,
        normal_sword: 14,
        freeze_attack_bonus: None,
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 2,
        attack: 90.0,
        interval: 7.0,
        normal_sword: 14,
        freeze_attack_bonus: None,
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 3,
        attack: 125.0,
        interval: 6.5,
        normal_sword: 14,
        freeze_attack_bonus: None,
        charged_enemy_sword: None,
    },
    StarProfile {
        star: 4,
        attack: 160.0,
        interval: 6.0,
        normal_sword: 14,
        freeze_attack_bonus: None,
        charged_enemy_sword: None,
    },
];

pub const SPEC: EquipmentSpec = EquipmentSpec {
    def: DEF,
    properties: PROPERTIES,
};
