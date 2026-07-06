use crate::Def;

pub mod items;

#[derive(Clone, Copy, Debug)]
pub enum Trigger {
    BattleStart,
    ActiveUse,
    NormalAttack,
    ChargedAttack,
    OnHeal,
    OnFreeze,
    OnHit,
    OnNormalHit,
    OnChargedHit,
    OnCriticalHit,
    OnParry,
    OnDisabled,
    Passive,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Target {
    SelfPlayer,
    EnemyPlayer,
    OwnAllWeapons,
    OwnAdjacentWeapons,
    OwnAllActiveItems,
    OwnAdjacentActiveItems,
    OwnRandomActiveItems(u8),
    EnemyRandomActiveItems(u8),
    EnemyAllActiveItems,
    ThisWeapon,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum DamageAmount {
    WeaponAttack,
    WeaponAttackMultiplier(f64),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Effect {
    MaxHp(i32),
    Heal(i32),
    Armor(i32),
    DamageReductionPct(f64),
    Damage(DamageAmount),
    Sword(i32),
    EnemySword(i32),
    ConsumeSword(i32),
    AttackBonus {
        target: Target,
        amount: i32,
        max_stacks: Option<u32>,
    },
    LifeStealPct {
        target: Target,
        pct: f64,
        max_stacks: Option<u32>,
    },
    Freeze {
        target: Target,
        seconds: f64,
    },
    Accelerate {
        target: Target,
        seconds: f64,
    },
    Charge {
        target: Target,
        seconds: f64,
    },
    Slow {
        target: Target,
        seconds: f64,
    },
    Uses(u8),
    ParryWindow {
        sword_cost: i32,
        stagger_seconds: f64,
    },
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct EffectSpec {
    pub trigger: Trigger,
    pub effects: &'static [Effect],
}

#[derive(Clone, Copy, Debug)]
pub struct EquipmentSpec {
    pub def: Def,
    pub properties: &'static [EffectSpec],
}

#[derive(Clone, Copy, Debug)]
pub struct StarProfile {
    pub star: u8,
    pub attack: f64,
    pub interval: f64,
    pub normal_sword: i32,
    pub freeze_attack_bonus: Option<i32>,
    pub charged_enemy_sword: Option<i32>,
}

pub fn specs() -> Vec<EquipmentSpec> {
    items::all()
}

pub fn star_profile(short: &str, star: u8) -> Option<StarProfile> {
    items::star_profile(short, star)
}
