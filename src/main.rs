use std::collections::{BTreeMap, HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{self, Read};

mod equipment;

use equipment::StarProfile;

const WIDTH: usize = 4;
const HEIGHT: usize = 5;
const MAX_TIME: f64 = 20.0;
const DT: f64 = 0.05;
const DEFAULT_RUNS: usize = 2400;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum Side {
    Me,
    Enemy,
}

impl Side {
    fn other(self) -> Self {
        match self {
            Side::Me => Side::Enemy,
            Side::Enemy => Side::Me,
        }
    }

    fn idx(self) -> usize {
        match self {
            Side::Me => 0,
            Side::Enemy => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Weapon,
    Charm,
    Potion,
    Soul,
    Armor,
}

#[derive(Clone, Copy, Debug)]
struct Def {
    short: &'static str,
    full: &'static str,
    kind: Kind,
    w: usize,
    h: usize,
    attack: f64,
    interval: f64,
}

fn defs() -> Vec<Def> {
    equipment::specs()
        .iter()
        .map(|spec| {
            let _property_count = spec.properties.len();
            spec.def
        })
        .collect()
}

#[derive(Clone)]
struct Item {
    def: Def,
    _star: Option<u8>,
    star_profile: Option<StarProfile>,
    cells: Vec<(usize, usize)>,
    attack_bonus: f64,
    progress: f64,
    frozen_until: f64,
    accelerated_until: f64,
    slowed_until: f64,
    stacks: HashMap<&'static str, u32>,
    used: bool,
}

impl Item {
    fn active(&self) -> bool {
        self.def.kind == Kind::Weapon || self.def.kind == Kind::Charm
    }

    fn weapon(&self) -> bool {
        self.def.kind == Kind::Weapon
    }

    fn attack(&self) -> f64 {
        self.star_profile
            .map(|profile| profile.attack)
            .unwrap_or(self.def.attack)
            + self.attack_bonus
    }

    fn interval(&self) -> f64 {
        self.star_profile
            .map(|profile| profile.interval)
            .unwrap_or(self.def.interval)
    }

    fn normal_sword_gain(&self, fallback: f64) -> f64 {
        self.star_profile
            .map(|profile| profile.normal_sword as f64)
            .unwrap_or(fallback)
    }

    fn freeze_attack_bonus(&self, fallback: f64) -> f64 {
        self.star_profile
            .and_then(|profile| profile.freeze_attack_bonus)
            .map(|value| value as f64)
            .unwrap_or(fallback)
    }

    fn charged_enemy_sword_loss(&self, fallback: f64) -> f64 {
        self.star_profile
            .and_then(|profile| profile.charged_enemy_sword)
            .map(|value| value as f64)
            .unwrap_or(fallback)
    }
}

#[derive(Clone)]
struct Player {
    items: Vec<Item>,
    hp: f64,
    max_hp: f64,
    armor: f64,
    damage_reduction: f64,
    sword: f64,
    stagger_until: f64,
}

impl Player {
    fn has(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.def.short == name)
    }
}

#[derive(Clone)]
struct Battle {
    players: [Player; 2],
    rng: Rng,
    events: Vec<Bucket>,
    timeline_events: Vec<TimelineEvent>,
    time: f64,
}

#[derive(Clone, Default)]
struct Bucket {
    hp_me_sum: f64,
    hp_enemy_sum: f64,
    samples: usize,
    damage_me_to_enemy: f64,
    damage_enemy_to_me: f64,
    heals_me: f64,
    heals_enemy: f64,
    freezes_me: usize,
    freezes_enemy: usize,
    parries_me: usize,
    parries_enemy: usize,
    attacks_me: usize,
    attacks_enemy: usize,
    reasons: BTreeMap<EventTag, CauseStat>,
}

#[derive(Clone, Default)]
struct CauseStat {
    amount: f64,
    count: usize,
}

#[derive(Clone)]
struct TimelineEvent {
    time_tick: u32,
    event: String,
    damage: Option<f64>,
    me_hp_after: f64,
    enemy_hp_after: f64,
    lethal_side: Option<Side>,
}

#[derive(Clone, Default)]
struct TimelineHpStat {
    sum: f64,
    samples: usize,
}

impl TimelineHpStat {
    fn add(&mut self, value: f64) {
        self.sum += value.max(0.0);
        self.samples += 1;
    }

    fn label(&self) -> String {
        if self.samples == 0 {
            "-".to_string()
        } else {
            format!("约 {:.0}", self.sum / self.samples as f64)
        }
    }
}

#[derive(Clone, Default)]
struct LethalStat {
    me: usize,
    enemy: usize,
}

impl LethalStat {
    fn add(&mut self, side: Option<Side>) {
        match side {
            Some(Side::Me) => self.me += 1,
            Some(Side::Enemy) => self.enemy += 1,
            None => {}
        }
    }

    fn label(&self, count: usize) -> String {
        if count == 0 {
            "-".to_string()
        } else if self.me * 2 >= count {
            "你阵亡".to_string()
        } else if self.enemy * 2 >= count {
            "敌方阵亡".to_string()
        } else {
            "-".to_string()
        }
    }

    fn is_majority_lethal(&self, count: usize) -> bool {
        count > 0 && (self.me * 2 >= count || self.enemy * 2 >= count)
    }
}

#[derive(Clone, Default)]
struct TimelineStat {
    damage_sum: f64,
    damage_count: usize,
    me_hp_after: TimelineHpStat,
    enemy_hp_after: TimelineHpStat,
    count: usize,
    lethal: LethalStat,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum EventTag {
    StartMaxHp {
        side: Side,
        source: &'static str,
    },
    StartSword {
        side: Side,
        source: &'static str,
    },
    StartArmor {
        side: Side,
        source: &'static str,
    },
    StartReduction {
        side: Side,
        source: &'static str,
    },
    ArmorGain {
        side: Side,
        source: &'static str,
    },
    AttackBoost {
        side: Side,
        source: &'static str,
    },
    Damage {
        attacker: Side,
        source: &'static str,
        target: Side,
    },
    ArmorAbsorb {
        attacker: Side,
        source: &'static str,
        target: Side,
    },
    Heal {
        side: Side,
        source: &'static str,
    },
    Freeze {
        source: Side,
        source_name: &'static str,
        target: Side,
    },
    Charge {
        side: Side,
        source: &'static str,
    },
    Accelerate {
        side: Side,
        source: &'static str,
    },
    Slow {
        side: Side,
        source: &'static str,
    },
    Stagger {
        side: Side,
        source: &'static str,
        target: Side,
    },
}

#[derive(Default)]
struct Summary {
    buckets: Vec<Bucket>,
    timeline: BTreeMap<(u32, String), TimelineStat>,
    wins_me: usize,
    wins_enemy: usize,
    draws: usize,
    avg_end_time: f64,
}

#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 7;
        x ^= x >> 9;
        x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = x;
        x
    }

    fn usize(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() as usize) % n
        }
    }

    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut runs = DEFAULT_RUNS;
    let mut seed = 20260706_u64;
    let mut input_path: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runs" => {
                runs = args
                    .next()
                    .ok_or("--runs 需要数字")?
                    .parse()
                    .map_err(|_| "--runs 不是有效数字")?
            }
            "--seed" => {
                seed = args
                    .next()
                    .ok_or("--seed 需要数字")?
                    .parse()
                    .map_err(|_| "--seed 不是有效数字")?
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            path => input_path = Some(path.to_string()),
        }
    }

    let text = if let Some(path) = input_path {
        fs::read_to_string(&path).map_err(|e| format!("读取 {path} 失败: {e}"))?
    } else {
        let mut s = String::new();
        io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("读取 stdin 失败: {e}"))?;
        if s.trim().is_empty() {
            sample_input().to_string()
        } else {
            s
        }
    };

    let (me_grid, enemy_grid) = parse_input(&text)?;
    let me_items = build_items(&me_grid)?;
    let enemy_items = build_items(&enemy_grid)?;
    let mut summary = Summary {
        buckets: vec![Bucket::default(); MAX_TIME as usize + 1],
        ..Default::default()
    };

    for run_idx in 0..runs {
        let battle = Battle::new(
            me_items.clone(),
            enemy_items.clone(),
            seed ^ ((run_idx as u64 + 1) * 0xA24B_AED4_963E_E407),
        );
        let (winner, end_time, buckets, timeline_events) = battle.simulate();
        match winner {
            Some(Side::Me) => summary.wins_me += 1,
            Some(Side::Enemy) => summary.wins_enemy += 1,
            None => summary.draws += 1,
        }
        summary.avg_end_time += end_time;
        merge_buckets(&mut summary.buckets, &buckets);
        merge_timeline(&mut summary.timeline, &timeline_events);
    }

    summary.avg_end_time /= runs as f64;
    print_report(&summary, runs, &me_items, &enemy_items);
    Ok(())
}

fn print_help() {
    println!("用法: cargo run --release -- [input.txt] [--runs 2400] [--seed 20260706]");
    println!("不传 input.txt 时读取 stdin；stdin 为空时使用内置示例。");
}

impl Battle {
    fn new(me_items: Vec<Item>, enemy_items: Vec<Item>, seed: u64) -> Self {
        let mut b = Self {
            players: [new_player(me_items), new_player(enemy_items)],
            rng: Rng::new(seed),
            events: vec![Bucket::default(); MAX_TIME as usize + 1],
            timeline_events: Vec::new(),
            time: 0.0,
        };
        b.apply_battle_start(Side::Me);
        b.apply_battle_start(Side::Enemy);
        b
    }

    fn simulate(mut self) -> (Option<Side>, f64, Vec<Bucket>, Vec<TimelineEvent>) {
        let steps = (MAX_TIME / DT).round() as usize;
        for step in 0..=steps {
            let t = step as f64 * DT;
            self.time = t;
            self.sample_hp(t);
            if self.players[0].hp <= 0.0 || self.players[1].hp <= 0.0 {
                self.fill_remaining_hp(step + 1, steps);
                return (self.winner(), t, self.events, self.timeline_events);
            }
            if step == steps {
                break;
            }
            if self.rng.bool() {
                self.tick_side(Side::Me, t);
                self.tick_side(Side::Enemy, t);
            } else {
                self.tick_side(Side::Enemy, t);
                self.tick_side(Side::Me, t);
            }
        }
        (
            self.winner_by_hp(),
            MAX_TIME,
            self.events,
            self.timeline_events,
        )
    }

    fn fill_remaining_hp(&mut self, start_step: usize, end_step: usize) {
        for step in start_step..=end_step {
            let t = step as f64 * DT;
            self.time = t;
            self.sample_hp(t);
        }
    }

    fn winner(&self) -> Option<Side> {
        let me_dead = self.players[0].hp <= 0.0;
        let enemy_dead = self.players[1].hp <= 0.0;
        match (me_dead, enemy_dead) {
            (true, false) => Some(Side::Enemy),
            (false, true) => Some(Side::Me),
            _ => None,
        }
    }

    fn winner_by_hp(&self) -> Option<Side> {
        let me = self.players[0].hp.max(0.0);
        let enemy = self.players[1].hp.max(0.0);
        if (me - enemy).abs() < 1.0 {
            None
        } else if me > enemy {
            Some(Side::Me)
        } else {
            Some(Side::Enemy)
        }
    }

    fn sample_hp(&mut self, t: f64) {
        let idx = bucket(t);
        self.events[idx].hp_me_sum += self.players[0].hp.max(0.0);
        self.events[idx].hp_enemy_sum += self.players[1].hp.max(0.0);
        self.events[idx].samples += 1;
    }

    fn tick_side(&mut self, side: Side, t: f64) {
        if self.players[side.idx()].hp <= 0.0 || self.players[side.idx()].stagger_until > t {
            return;
        }
        let mut ready = Vec::new();
        let len = self.players[side.idx()].items.len();
        for i in 0..len {
            let item = &mut self.players[side.idx()].items[i];
            if !item.active() || item.frozen_until > t {
                continue;
            }
            let mut speed = 1.0;
            if item.accelerated_until > t {
                speed += 1.0;
            }
            if item.slowed_until > t {
                speed -= 0.5;
            }
            if speed <= 0.0 {
                continue;
            }
            item.progress += DT * speed;
            let interval = item.interval();
            if item.progress + 1e-9 >= interval {
                item.progress -= interval;
                ready.push(i);
            }
        }
        for idx in ready {
            if self.players[side.idx()].hp <= 0.0 || self.players[side.other().idx()].hp <= 0.0 {
                break;
            }
            if idx >= self.players[side.idx()].items.len() {
                continue;
            }
            match self.players[side.idx()].items[idx].def.short {
                "冰魄" => {
                    let sword_gain = self.players[side.idx()].items[idx].normal_sword_gain(16.0);
                    self.normal_attack(side, idx, sword_gain, "冰魄普通攻击", true);
                }
                "龙弧" => self.longhu_attack(side, idx),
                "烛神令" => self.zhushenling_attack(side, idx),
                "冰髓" => {
                    self.freeze_random(side, side.other(), 2, 1.5, t, "冰髓");
                    self.players[side.idx()].sword += 10.0;
                }
                "寒冰核" => self.freeze_random(side, side.other(), 1, 3.5, t, "寒冰核"),
                "缚命环" => {
                    self.players[side.idx()].items[idx].used = true;
                    self.heal(side, 90.0, "缚命环");
                }
                "蜃景苞" => {
                    self.freeze_random(side, side.other(), 1, 1.1, t, "蜃景苞");
                    self.charge_random(side, 1.5);
                }
                _ => {}
            }
        }
    }

    fn normal_attack(
        &mut self,
        side: Side,
        item_idx: usize,
        sword_gain: f64,
        cause: &'static str,
        ice_po: bool,
    ) {
        let dmg = self.players[side.idx()].items[item_idx].attack();
        self.deal_damage(side, dmg, cause);
        self.count_attack(side);
        self.players[side.idx()].sword += sword_gain;
        if self.players[side.idx()].has("混沌") {
            self.players[side.idx()].sword += 8.0;
        }
        self.on_attack_hit(side);
        self.on_normal_hit(side);
        if ice_po {
            self.freeze_random(side, side.other(), 2, 2.0, 0.0, "冰魄");
        }
    }

    fn longhu_attack(&mut self, side: Side, item_idx: usize) {
        if self.players[side.idx()].sword >= 28.0 {
            self.players[side.idx()].sword -= 28.0;
            if self.try_parry(side) {
                return;
            }
            self.count_attack(side);
            let dmg = self.players[side.idx()].items[item_idx].attack() * 3.7;
            self.deal_damage(side, dmg, "龙弧蓄力攻击");
            let enemy_sword_loss =
                self.players[side.idx()].items[item_idx].charged_enemy_sword_loss(16.0);
            self.players[side.other().idx()].sword =
                (self.players[side.other().idx()].sword - enemy_sword_loss).max(0.0);
            self.on_attack_hit(side);
            self.on_charged_hit(side);
            self.freeze_random(side, side.other(), 1, 2.0, 0.0, "龙弧");
        } else {
            let sword_gain = self.players[side.idx()].items[item_idx].normal_sword_gain(16.0);
            self.normal_attack(side, item_idx, sword_gain, "龙弧普通攻击", false);
        }
    }

    fn zhushenling_attack(&mut self, side: Side, item_idx: usize) {
        if self.players[side.idx()].sword >= 24.0 {
            self.players[side.idx()].sword -= 24.0;
            if self.try_parry(side) {
                return;
            }
            self.count_attack(side);
            let dmg = self.players[side.idx()].items[item_idx].attack() * 2.25;
            self.deal_damage(side, dmg, "烛神令蓄力攻击");
            self.heal(side, 4.0, "烛神令生命恢复");
            self.on_attack_hit(side);
            self.on_charged_hit(side);
        } else {
            self.count_attack(side);
            let dmg = self.players[side.idx()].items[item_idx].attack();
            self.deal_damage(side, dmg, "烛神令普通攻击");
            self.players[side.idx()].sword +=
                self.players[side.idx()].items[item_idx].normal_sword_gain(14.0);
            self.heal(side, 4.0, "烛神令生命恢复");
            self.on_attack_hit(side);
            self.on_normal_hit(side);
        }
    }

    fn try_parry(&mut self, attacker: Side) -> bool {
        let defender = attacker.other();
        if self.players[defender.idx()].sword >= 50.0 {
            self.players[defender.idx()].sword -= 50.0;
            self.players[attacker.idx()].stagger_until = self.nowish() + 3.0;
            let b = self.cur_bucket();
            if defender == Side::Me {
                self.events[b].parries_me += 1;
            } else {
                self.events[b].parries_enemy += 1;
            }
            self.record_reason(
                EventTag::Stagger {
                    side: defender,
                    source: "振刀",
                    target: attacker,
                },
                1.0,
            );
            self.record_timeline_event(parry_event_text(defender, attacker), None, None);
            self.on_parry_success(defender);
            self.on_own_item_disabled(attacker);
            true
        } else {
            false
        }
    }

    fn count_attack(&mut self, side: Side) {
        let b = self.cur_bucket();
        if side == Side::Me {
            self.events[b].attacks_me += 1;
        } else {
            self.events[b].attacks_enemy += 1;
        }
    }

    fn deal_damage(&mut self, attacker: Side, raw: f64, cause: &'static str) {
        let defender = attacker.other();
        let will_trigger_white_tiger = self.will_trigger_white_tiger(attacker, cause);
        let p = &mut self.players[defender.idx()];
        let dmg = raw * (1.0 - p.damage_reduction);
        let from_armor = p.armor.min(dmg);
        p.armor -= from_armor;
        let hp_loss = dmg - from_armor;
        p.hp -= hp_loss;
        let b = self.cur_bucket();
        if attacker == Side::Me {
            self.events[b].damage_me_to_enemy += dmg;
        } else {
            self.events[b].damage_enemy_to_me += dmg;
        }
        if hp_loss > 0.0 {
            self.record_reason(
                EventTag::Damage {
                    attacker,
                    source: cause,
                    target: defender,
                },
                hp_loss,
            );
            self.record_timeline_damage(
                attacker,
                defender,
                cause,
                hp_loss,
                will_trigger_white_tiger,
            );
        }
        if from_armor > 0.0 {
            self.record_reason(
                EventTag::ArmorAbsorb {
                    attacker,
                    source: cause,
                    target: defender,
                },
                from_armor,
            );
        }
        if self.players[defender.idx()].has("冻结") {
            self.freeze_random(defender, attacker, 1, 2.0, 0.0, "冻结魂玉");
        }
    }

    fn will_trigger_white_tiger(&self, attacker: Side, _cause: &'static str) -> bool {
        self.find_first(attacker, "白虎之力")
            .is_some_and(|idx| !self.players[attacker.idx()].items[idx].used)
    }

    fn record_timeline_damage(
        &mut self,
        attacker: Side,
        defender: Side,
        cause: &'static str,
        damage: f64,
        will_trigger_white_tiger: bool,
    ) {
        let mut event = damage_event_text(attacker, cause);
        if will_trigger_white_tiger {
            if attacker == Side::Enemy {
                event.push_str("，触发 白虎之力，同时冻结你所有道具 3 秒");
            } else {
                event.push_str("，触发 白虎之力，同时冻结敌方所有道具 3 秒");
            }
        } else if cause.contains("冰魄") {
            if attacker == Side::Enemy {
                event.push_str("并继续冻结你方道具");
            } else {
                event.push_str("并继续冻结敌方道具");
            }
        }
        let lethal_side = if self.players[defender.idx()].hp <= 0.0 {
            Some(defender)
        } else {
            None
        };
        self.record_timeline_event(event, Some(damage), lethal_side);
    }

    fn record_timeline_event(
        &mut self,
        event: String,
        damage: Option<f64>,
        lethal_side: Option<Side>,
    ) {
        let me_hp_after = self.players[Side::Me.idx()].hp.max(0.0);
        let enemy_hp_after = self.players[Side::Enemy.idx()].hp.max(0.0);
        self.timeline_events.push(TimelineEvent {
            time_tick: (self.time * 100.0).round() as u32,
            event,
            damage,
            me_hp_after,
            enemy_hp_after,
            lethal_side,
        });
    }

    fn heal(&mut self, side: Side, amount: f64, cause: &'static str) {
        if amount <= 0.0 {
            return;
        }
        let p = &mut self.players[side.idx()];
        let before = p.hp;
        p.hp = (p.hp + amount).min(p.max_hp);
        let actual = p.hp - before;
        let b = self.cur_bucket();
        if side == Side::Me {
            self.events[b].heals_me += actual;
        } else {
            self.events[b].heals_enemy += actual;
        }
        if actual > 0.0 {
            self.record_reason(
                EventTag::Heal {
                    side,
                    source: cause,
                },
                actual,
            );
        }
        self.on_heal(side);
    }

    fn freeze_random(
        &mut self,
        source: Side,
        target: Side,
        count: usize,
        duration: f64,
        t: f64,
        source_name: &'static str,
    ) {
        let active: Vec<usize> = self.players[target.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.active())
            .map(|(i, _)| i)
            .collect();
        if active.is_empty() {
            return;
        }
        let mut pool = active;
        for _ in 0..count.min(pool.len()) {
            let pick = self.rng.usize(pool.len());
            let idx = pool.swap_remove(pick);
            let until = self.nowish().max(t) + duration;
            self.players[target.idx()].items[idx].frozen_until = self.players[target.idx()].items
                [idx]
                .frozen_until
                .max(until);
            let b = self.cur_bucket();
            if source == Side::Me {
                self.events[b].freezes_me += 1;
            } else {
                self.events[b].freezes_enemy += 1;
            }
            self.record_reason(
                EventTag::Freeze {
                    source,
                    source_name,
                    target,
                },
                1.0,
            );
            self.on_own_item_disabled(target);
        }
        self.on_freeze_triggered(source, source_name);
    }

    fn freeze_all(&mut self, source: Side, target: Side, duration: f64) {
        let len = self.players[target.idx()].items.len();
        for i in 0..len {
            if self.players[target.idx()].items[i].active() {
                self.players[target.idx()].items[i].frozen_until = self.players[target.idx()].items
                    [i]
                    .frozen_until
                    .max(self.nowish() + duration);
                let b = self.cur_bucket();
                if source == Side::Me {
                    self.events[b].freezes_me += 1;
                } else {
                    self.events[b].freezes_enemy += 1;
                }
                self.record_reason(
                    EventTag::Freeze {
                        source,
                        source_name: "白虎之力",
                        target,
                    },
                    1.0,
                );
                self.on_own_item_disabled(target);
            }
        }
        self.on_freeze_triggered(source, "白虎之力");
    }

    fn on_freeze_triggered(&mut self, side: Side, source_name: &'static str) {
        if self.players[side.idx()].has("续命术") {
            self.heal(side, 70.0, "续命术");
        }
        let quick_cells: Vec<Vec<(usize, usize)>> = self.players[side.idx()]
            .items
            .iter()
            .filter(|i| i.def.short == "快意")
            .map(|i| i.cells.clone())
            .collect();
        for cells in quick_cells {
            let targets = self.adjacent_active_items(side, &cells, false);
            for idx in targets {
                self.accelerate_item(side, idx, 4.0, "快意魂玉");
            }
        }
        if source_name == "冰魄" {
            self.add_icepo_stack(side);
        }
    }

    fn on_attack_hit(&mut self, side: Side) {
        if self.players[side.idx()].has("破势") {
            self.players[side.other().idx()].sword =
                (self.players[side.other().idx()].sword - 12.0).max(0.0);
        }
        if self.players[side.idx()].has("锋锐") {
            self.slow_random(side.other(), 4.0);
        }
        if self.players[side.idx()]
            .items
            .iter()
            .any(|item| item.def.short == "缚命环" && item.used)
        {
            self.heal(side, 40.0, "缚命环命中治疗");
        }
        if let Some(idx) = self.find_first(side, "白虎之力") {
            if !self.players[side.idx()].items[idx].used {
                self.players[side.idx()].items[idx].used = true;
                self.freeze_all(side, side.other(), 3.0);
                self.add_weapon_attack(side, 100.0);
            }
        }
    }

    fn on_normal_hit(&mut self, side: Side) {
        let xu = self.find_first(side, "玄武");
        if let Some(idx) = xu {
            if !self.players[side.idx()].items[idx].used {
                self.players[side.idx()].items[idx].used = true;
                self.players[side.idx()].max_hp += 200.0;
                self.record_reason(
                    EventTag::StartMaxHp {
                        side,
                        source: "玄武之力",
                    },
                    200.0,
                );
                self.heal(side, 200.0, "玄武之力");
                self.add_weapon_attack(side, 40.0);
                self.accelerate_weapons(side, 3.0, "玄武之力");
            }
        }
        if self.players[side.idx()].has("寒冰核") {
            self.slow_random(side.other(), 1.0);
        }
        if self.players[side.idx()].has("催破") {
            self.accelerate_random(side, 4.0, "催破魂玉");
        }
    }

    fn on_charged_hit(&mut self, side: Side) {
        if self.players[side.idx()].has("振魄") {
            self.players[side.idx()].sword += 20.0;
            self.freeze_random(side, side.other(), 1, 1.0, 0.0, "振魄");
        }
    }

    fn on_heal(&mut self, side: Side) {
        let len = self.players[side.idx()].items.len();
        for i in 0..len {
            match self.players[side.idx()].items[i].def.short {
                "百战" => {
                    if self.bump_stack(side, i, "百战", 10) {
                        self.add_weapon_attack(side, 16.0);
                    }
                }
                "吐纳" => {
                    if self.bump_stack(side, i, "吐纳", 20) {
                        let cells = self.players[side.idx()].items[i].cells.clone();
                        for w in self.adjacent_weapons(side, &cells) {
                            self.players[side.idx()].items[w].attack_bonus += 15.0;
                        }
                    }
                }
                "战神" => {
                    let cells = self.players[side.idx()].items[i].cells.clone();
                    for w in self.adjacent_weapons(side, &cells) {
                        self.accelerate_item(side, w, 2.5, "战神魂玉");
                    }
                }
                "凌波" => self.slow_random(side.other(), 4.0),
                "铸盾" => self.players[side.idx()].armor += 45.0,
                "烛神令" => self.players[side.idx()].items[i].progress += 1.0,
                _ => {}
            }
        }
    }

    fn on_parry_success(&mut self, side: Side) {
        if self.players[side.idx()].has("天响") {
            self.charge_all_active(side, 1.5);
        }
        if self.players[side.idx()].has("混沌") {
            self.accelerate_weapons(side, 4.0, "混沌魂玉");
        }
    }

    fn on_own_item_disabled(&mut self, side: Side) {
        let count = self.players[side.idx()]
            .items
            .iter()
            .filter(|i| i.def.short == "神将甲")
            .count();
        for _ in 0..count {
            self.players[side.idx()].armor += 15.0;
            self.record_reason(
                EventTag::ArmorGain {
                    side,
                    source: "神将甲",
                },
                15.0,
            );
            self.heal(side, 15.0, "神将甲");
        }
    }

    fn apply_battle_start(&mut self, side: Side) {
        let len = self.players[side.idx()].items.len();
        for i in 0..len {
            match self.players[side.idx()].items[i].def.short {
                "百战" => {
                    self.players[side.idx()].max_hp += 180.0;
                    self.players[side.idx()].hp += 180.0;
                    self.record_reason(
                        EventTag::StartMaxHp {
                            side,
                            source: "百战魂玉",
                        },
                        180.0,
                    );
                }
                "天响" => {
                    self.players[side.idx()].sword += 22.0;
                    self.record_reason(
                        EventTag::StartSword {
                            side,
                            source: "天响魂玉",
                        },
                        22.0,
                    );
                }
                "通达" => self.accelerate_all_active(side, 2.0, "通达魂玉"),
                "穿颅" => {
                    self.accelerate_weapons(side, 6.0, "穿颅魂玉");
                    let cells = self.players[side.idx()].items[i].cells.clone();
                    let targets = self.adjacent_active_items(side, &cells, false);
                    for idx in targets {
                        self.players[side.idx()].items[idx].progress += 3.0;
                        self.record_reason(
                            EventTag::Charge {
                                side,
                                source: "穿颅魂玉",
                            },
                            3.0,
                        );
                    }
                }
                "神将甲" => {
                    self.players[side.idx()].armor += 300.0;
                    self.players[side.idx()].damage_reduction += 0.05;
                    self.record_reason(
                        EventTag::StartArmor {
                            side,
                            source: "神将甲护甲",
                        },
                        300.0,
                    );
                    self.record_reason(
                        EventTag::StartReduction {
                            side,
                            source: "神将甲减伤",
                        },
                        5.0,
                    );
                }
                "振魄" | "混沌" => {
                    self.players[side.idx()].sword += 20.0;
                    self.record_reason(
                        EventTag::StartSword {
                            side,
                            source: self.players[side.idx()].items[i].def.full,
                        },
                        20.0,
                    );
                }
                "烛神令" => {
                    self.players[side.idx()].sword += 20.0;
                    self.record_reason(
                        EventTag::StartSword {
                            side,
                            source: "烛神令",
                        },
                        20.0,
                    );
                }
                _ => {}
            }
        }
    }

    fn bump_stack(&mut self, side: Side, idx: usize, key: &'static str, max: u32) -> bool {
        let item = &mut self.players[side.idx()].items[idx];
        let cur = *item.stacks.get(key).unwrap_or(&0);
        if cur >= max {
            false
        } else {
            item.stacks.insert(key, cur + 1);
            true
        }
    }

    fn add_icepo_stack(&mut self, side: Side) {
        let cur = self.players[side.idx()]
            .items
            .iter()
            .find(|i| i.def.short == "冰魄")
            .and_then(|i| i.stacks.get("冰魄全局"))
            .copied()
            .unwrap_or(0);
        if cur >= 7 {
            return;
        }
        for item in &mut self.players[side.idx()].items {
            if item.def.short == "冰魄" {
                item.stacks.insert("冰魄全局", cur + 1);
            }
        }
        let amount = self.players[side.idx()]
            .items
            .iter()
            .filter(|i| i.def.short == "冰魄")
            .map(|i| i.freeze_attack_bonus(35.0))
            .fold(35.0, f64::max);
        self.add_weapon_attack(side, amount);
    }

    fn add_weapon_attack(&mut self, side: Side, amount: f64) {
        for item in &mut self.players[side.idx()].items {
            if item.weapon() {
                item.attack_bonus += amount;
            }
        }
        self.record_reason(
            EventTag::AttackBoost {
                side,
                source: "所有武器攻击力提升",
            },
            amount,
        );
    }

    fn accelerate_all_active(&mut self, side: Side, duration: f64, source: &'static str) {
        let idxs: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.active())
            .map(|(i, _)| i)
            .collect();
        for idx in idxs {
            self.accelerate_item(side, idx, duration, source);
        }
    }

    fn accelerate_weapons(&mut self, side: Side, duration: f64, source: &'static str) {
        let idxs: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.weapon())
            .map(|(i, _)| i)
            .collect();
        for idx in idxs {
            self.accelerate_item(side, idx, duration, source);
        }
    }

    fn accelerate_item(&mut self, side: Side, idx: usize, duration: f64, source: &'static str) {
        self.players[side.idx()].items[idx].accelerated_until = self.players[side.idx()].items[idx]
            .accelerated_until
            .max(self.nowish() + duration);
        self.record_reason(EventTag::Accelerate { side, source }, duration);
        self.on_accelerated(side, idx);
    }

    fn on_accelerated(&mut self, side: Side, target_idx: usize) {
        let target_cells = self.players[side.idx()].items[target_idx].cells.clone();
        let tongda: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.def.short == "通达")
            .map(|(i, _)| i)
            .collect();
        for idx in tongda {
            if adjacent(&self.players[side.idx()].items[idx].cells, &target_cells)
                && self.bump_stack(side, idx, "通达", 20)
            {
                let cells = self.players[side.idx()].items[idx].cells.clone();
                for w in self.adjacent_weapons(side, &cells) {
                    self.players[side.idx()].items[w].attack_bonus += 18.0;
                }
                self.record_reason(
                    EventTag::AttackBoost {
                        side,
                        source: "通达魂玉",
                    },
                    18.0,
                );
            }
        }
    }

    fn charge_all_active(&mut self, side: Side, amount: f64) {
        for item in &mut self.players[side.idx()].items {
            if item.active() {
                item.progress += amount;
            }
        }
        self.record_reason(
            EventTag::Charge {
                side,
                source: "所有读条装备充能",
            },
            amount,
        );
    }

    fn charge_random(&mut self, side: Side, amount: f64) {
        let active: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.active())
            .map(|(i, _)| i)
            .collect();
        if !active.is_empty() {
            let idx = active[self.rng.usize(active.len())];
            self.players[side.idx()].items[idx].progress += amount;
            self.record_reason(
                EventTag::Charge {
                    side,
                    source: "随机充能",
                },
                amount,
            );
        }
    }

    fn accelerate_random(&mut self, side: Side, duration: f64, source: &'static str) {
        let active: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.active())
            .map(|(i, _)| i)
            .collect();
        if !active.is_empty() {
            let idx = active[self.rng.usize(active.len())];
            self.accelerate_item(side, idx, duration, source);
        }
    }

    fn slow_random(&mut self, side: Side, duration: f64) {
        let active: Vec<usize> = self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.active())
            .map(|(i, _)| i)
            .collect();
        if !active.is_empty() {
            let idx = active[self.rng.usize(active.len())];
            self.players[side.idx()].items[idx].slowed_until = self.players[side.idx()].items[idx]
                .slowed_until
                .max(self.nowish() + duration);
            self.record_reason(
                EventTag::Slow {
                    side,
                    source: "减速",
                },
                1.0,
            );
        }
    }

    fn adjacent_weapons(&self, side: Side, cells: &[(usize, usize)]) -> Vec<usize> {
        self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.weapon() && adjacent(cells, &i.cells))
            .map(|(i, _)| i)
            .collect()
    }

    fn adjacent_active_items(
        &self,
        side: Side,
        cells: &[(usize, usize)],
        include_self: bool,
    ) -> Vec<usize> {
        self.players[side.idx()]
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                i.active() && adjacent(cells, &i.cells) && (include_self || i.cells != cells)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn find_first(&self, side: Side, short: &str) -> Option<usize> {
        self.players[side.idx()]
            .items
            .iter()
            .position(|i| i.def.short == short)
    }

    fn nowish(&self) -> f64 {
        self.time
    }

    fn cur_bucket(&self) -> usize {
        bucket(self.time)
    }

    fn record_reason(&mut self, tag: EventTag, amount: f64) {
        if let Some(event) = timeline_reason_text(&tag) {
            self.record_timeline_event(event, None, None);
        }
        let b = self.cur_bucket();
        let stat = self.events[b].reasons.entry(tag).or_default();
        stat.amount += amount;
        stat.count += 1;
    }
}

fn new_player(items: Vec<Item>) -> Player {
    Player {
        items,
        hp: 1000.0,
        max_hp: 1000.0,
        armor: 0.0,
        damage_reduction: 0.0,
        sword: 0.0,
        stagger_until: 0.0,
    }
}

fn bucket(t: f64) -> usize {
    t.floor().clamp(0.0, MAX_TIME) as usize
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Me => "我方",
        Side::Enemy => "敌方",
    }
}

fn merge_buckets(dst: &mut [Bucket], src: &[Bucket]) {
    for (d, s) in dst.iter_mut().zip(src) {
        d.hp_me_sum += s.hp_me_sum;
        d.hp_enemy_sum += s.hp_enemy_sum;
        d.samples += s.samples;
        d.damage_me_to_enemy += s.damage_me_to_enemy;
        d.damage_enemy_to_me += s.damage_enemy_to_me;
        d.heals_me += s.heals_me;
        d.heals_enemy += s.heals_enemy;
        d.freezes_me += s.freezes_me;
        d.freezes_enemy += s.freezes_enemy;
        d.parries_me += s.parries_me;
        d.parries_enemy += s.parries_enemy;
        d.attacks_me += s.attacks_me;
        d.attacks_enemy += s.attacks_enemy;
        for (reason, stat) in &s.reasons {
            let dst_stat = d.reasons.entry(reason.clone()).or_default();
            dst_stat.amount += stat.amount;
            dst_stat.count += stat.count;
        }
    }
}

fn merge_timeline(dst: &mut BTreeMap<(u32, String), TimelineStat>, src: &[TimelineEvent]) {
    for event in src {
        let stat = dst
            .entry((event.time_tick, event.event.clone()))
            .or_default();
        if let Some(damage) = event.damage {
            stat.damage_sum += damage;
            stat.damage_count += 1;
        }
        stat.me_hp_after.add(event.me_hp_after);
        stat.enemy_hp_after.add(event.enemy_hp_after);
        stat.count += 1;
        stat.lethal.add(event.lethal_side);
    }
}

fn print_report(summary: &Summary, runs: usize, _me_items: &[Item], _enemy_items: &[Item]) {
    let winner = if summary.wins_me > summary.wins_enemy {
        "你这套"
    } else if summary.wins_enemy > summary.wins_me {
        "敌方"
    } else {
        "双方接近"
    };
    let top_wins = summary.wins_me.max(summary.wins_enemy);
    let stable = if top_wins as f64 / runs as f64 >= 0.8 {
        "，而且是比较稳定的胜"
    } else {
        "，但胜负波动较大"
    };

    println!(
        "程序模拟结果：{}胜{}。按当前规则跑了 {} 次随机冻结目标，结果是：",
        winner, stable, runs
    );
    println!("| 阵容 | 胜场 |");
    println!("|---|---:|");
    println!("| 你这套 | {} |", summary.wins_me);
    println!("| 敌方这套 | {} |", summary.wins_enemy);
    if summary.draws > 0 {
        println!("| 平局/判平 | {} |", summary.draws);
    }
    println!();
    println!("核心时间线大概是：");
    println!("| 时间 | 事件 | 平均伤害 | 你的剩余血量 | 敌方剩余血量 | 击杀 |");
    println!("|---:|---|---:|---:|---:|---|");
    for row in core_timeline(summary, runs) {
        println!(
            "| {} | {} | {} | {} | {} | {} |",
            row.time, row.event, row.damage, row.me_hp, row.enemy_hp, row.lethal
        );
    }
}

struct TimelineRow {
    time: String,
    event: String,
    damage: String,
    me_hp: String,
    enemy_hp: String,
    lethal: String,
}

fn core_timeline(summary: &Summary, runs: usize) -> Vec<TimelineRow> {
    let mut rows = Vec::new();
    let min_count = (runs as f64 * 0.05).ceil() as usize;
    for ((time_tick, event), stat) in &summary.timeline {
        if stat.count < min_count {
            continue;
        }
        rows.push(TimelineRow {
            time: format!("{:.2}s", *time_tick as f64 / 100.0),
            event: event.clone(),
            damage: if stat.damage_count == 0 {
                "-".to_string()
            } else {
                format!("{:.0}", stat.damage_sum / stat.damage_count as f64)
            },
            me_hp: stat.me_hp_after.label(),
            enemy_hp: stat.enemy_hp_after.label(),
            lethal: stat.lethal.label(stat.count),
        });
        if stat.lethal.is_majority_lethal(stat.count) {
            break;
        }
    }
    rows
}

fn damage_event_text(attacker: Side, cause: &'static str) -> String {
    let side = match attacker {
        Side::Me => "你",
        Side::Enemy => "敌方",
    };
    let action = match cause {
        "龙弧蓄力攻击" => "龙弧 蓄力命中",
        "龙弧普通攻击" => "龙弧 普攻命中",
        "烛神令蓄力攻击" => "烛神令 蓄力命中",
        "烛神令普通攻击" => "烛神令 普攻命中",
        "冰魄普通攻击" => "冰魄 普攻命中",
        other => other,
    };
    format!("{side} {action}")
}

fn parry_event_text(defender: Side, attacker: Side) -> String {
    match (defender, attacker) {
        (Side::Me, Side::Enemy) => "你振刀成功，敌方硬直 3 秒".to_string(),
        (Side::Enemy, Side::Me) => "敌方振刀成功，你方硬直 3 秒".to_string(),
        _ => "振刀成功，攻击方硬直 3 秒".to_string(),
    }
}

fn timeline_reason_text(reason: &EventTag) -> Option<String> {
    match reason {
        EventTag::StartMaxHp { .. }
        | EventTag::StartSword { .. }
        | EventTag::StartArmor { .. }
        | EventTag::StartReduction { .. }
        | EventTag::Damage { .. }
        | EventTag::ArmorAbsorb { .. }
        | EventTag::Stagger { .. } => None,
        EventTag::ArmorGain { side, source } => {
            Some(format!("{}{}触发护甲提升", timeline_side(*side), source))
        }
        EventTag::AttackBoost { side, source } => {
            if source.contains("攻击力") {
                Some(format!("{}{}", timeline_side(*side), source))
            } else {
                Some(format!("{}{}提升武器攻击力", timeline_side(*side), source))
            }
        }
        EventTag::Heal { side, source } => {
            Some(format!("{}{}触发治疗", timeline_side(*side), source))
        }
        EventTag::Freeze {
            source,
            source_name,
            target,
        } => Some(format!(
            "{}{}冻结{}读条道具",
            timeline_side(*source),
            source_name,
            timeline_target(*target)
        )),
        EventTag::Charge { side, source } => {
            if source.contains("充能") {
                Some(format!("{}{}", timeline_side(*side), source))
            } else {
                Some(format!("{}{}推进读条", timeline_side(*side), source))
            }
        }
        EventTag::Accelerate { side, source } => {
            Some(format!("{}{}触发加速", timeline_side(*side), source))
        }
        EventTag::Slow { side, source } => {
            Some(format!("{}{}触发减速", timeline_side(*side), source))
        }
    }
}

fn timeline_side(side: Side) -> &'static str {
    match side {
        Side::Me => "你方",
        Side::Enemy => "敌方",
    }
}

fn timeline_target(side: Side) -> &'static str {
    match side {
        Side::Me => "你方",
        Side::Enemy => "敌方",
    }
}

#[allow(dead_code)]
fn render_battle_step(sec: usize, bucket: &Bucket, prev: Option<&Bucket>, divisor: f64) -> String {
    let me_hp = bucket.hp_me_sum / bucket.samples as f64;
    let enemy_hp = bucket.hp_enemy_sum / bucket.samples as f64;
    let (prev_me_hp, prev_enemy_hp) = prev
        .map(|p| {
            (
                p.hp_me_sum / p.samples.max(1) as f64,
                p.hp_enemy_sum / p.samples.max(1) as f64,
            )
        })
        .unwrap_or((1000.0, 1000.0));
    let me_delta = me_hp - prev_me_hp;
    let enemy_delta = enemy_hp - prev_enemy_hp;

    let mut lines = Vec::new();
    lines.push(format!(
        "[{sec:>2}s] 我方HP {:.1} ({:+.1})，敌方HP {:.1} ({:+.1})；我->敌伤害 {:.1}，敌->我伤害 {:.1}，我方治疗 {:.1}，敌方治疗 {:.1}",
        me_hp,
        me_delta,
        enemy_hp,
        enemy_delta,
        bucket.damage_me_to_enemy / divisor,
        bucket.damage_enemy_to_me / divisor,
        bucket.heals_me / divisor,
        bucket.heals_enemy / divisor,
    ));

    let reasons = top_reasons(bucket, 5);
    if !reasons.is_empty() {
        let mut cause_parts = Vec::new();
        for (reason, stat) in reasons {
            cause_parts.push(format!(
                "{}（均值 {:.1}，触发 {:.2} 次）",
                narrate_event(reason),
                stat.amount / divisor,
                stat.count as f64 / divisor
            ));
        }
        lines.push(format!("    主要经过：{}", cause_parts.join("；")));
    }

    let control = format!(
        "    控制/节奏：我方冻结 {:.2} 次，敌方冻结 {:.2} 次，我方振刀 {:.2} 次，敌方振刀 {:.2} 次",
        bucket.freezes_me as f64 / divisor,
        bucket.freezes_enemy as f64 / divisor,
        bucket.parries_me as f64 / divisor,
        bucket.parries_enemy as f64 / divisor
    );
    lines.push(control);

    if sec >= 2
        && me_delta.abs() < 0.1
        && enemy_delta.abs() < 0.1
        && bucket.damage_me_to_enemy == 0.0
        && bucket.damage_enemy_to_me == 0.0
        && bucket.heals_me == 0.0
        && bucket.heals_enemy == 0.0
    {
        lines.push("    战局已经结束，后续只是血线维持，没有新的交互。".to_string());
    }

    lines.join("\n")
}

fn narrate_event(reason: &EventTag) -> String {
    match reason {
        EventTag::StartMaxHp { side, source } => {
            format!("{}{}抬高了最大生命", side_name(*side), source)
        }
        EventTag::StartSword { side, source } => {
            format!("{}{}补了初始剑势", side_name(*side), source)
        }
        EventTag::StartArmor { side, source } => {
            format!("{}{}提供了开局护甲", side_name(*side), source)
        }
        EventTag::StartReduction { side, source } => {
            format!("{}{}提供了减伤", side_name(*side), source)
        }
        EventTag::ArmorGain { side, source } => {
            format!("{}{}触发护甲提升", side_name(*side), source)
        }
        EventTag::AttackBoost { side, source } => {
            format!("{}{}抬高了武器攻击力", side_name(*side), source)
        }
        EventTag::Damage {
            attacker,
            source,
            target,
        } => format!(
            "{}{}对{}造成伤害",
            side_name(*attacker),
            source,
            side_name(*target)
        ),
        EventTag::ArmorAbsorb {
            attacker,
            source,
            target,
        } => format!(
            "{}{}被{}护甲吸收",
            side_name(*attacker),
            source,
            side_name(*target)
        ),
        EventTag::Heal { side, source } => format!("{}{}触发回复", side_name(*side), source),
        EventTag::Freeze {
            source,
            source_name,
            target,
        } => format!(
            "{}{}冻结了{}的读条装备",
            side_name(*source),
            source_name,
            side_name(*target)
        ),
        EventTag::Charge { side, source } => format!("{}{}推进了读条", side_name(*side), source),
        EventTag::Accelerate { side, source } => {
            format!("{}{}让读条加速", side_name(*side), source)
        }
        EventTag::Slow { side, source } => format!("{}{}造成减速", side_name(*side), source),
        EventTag::Stagger {
            side,
            source,
            target,
        } => format!("{}{}让{}硬直", side_name(*side), source, side_name(*target)),
    }
}

fn top_reasons(bucket: &Bucket, limit: usize) -> Vec<(&EventTag, &CauseStat)> {
    let mut reasons: Vec<(&EventTag, &CauseStat)> = bucket.reasons.iter().collect();
    reasons.sort_by(|a, b| {
        let a_score = a.1.amount.abs().max(a.1.count as f64);
        let b_score = b.1.amount.abs().max(b.1.count as f64);
        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.count.cmp(&a.1.count))
    });
    reasons.truncate(limit);
    reasons
}

fn parse_input(
    text: &str,
) -> Result<([[String; WIDTH]; HEIGHT], [[String; WIDTH]; HEIGHT]), String> {
    let mut sections: [Vec<Vec<String>>; 2] = [Vec::new(), Vec::new()];
    let mut cur: Option<usize> = None;
    let all_defs = defs();
    let names: Vec<&str> = all_defs.iter().map(|d| d.short).collect();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("```") || trimmed.contains("行 / 列") {
            continue;
        }
        if trimmed.contains("现在") || trimmed.contains("我这套") || trimmed.contains("我方")
        {
            cur = Some(0);
            continue;
        }
        if trimmed.contains("敌方") {
            cur = Some(1);
            continue;
        }
        let Some(section) = cur else { continue };
        let cells = parse_row(trimmed, &names);
        if cells.len() == WIDTH {
            sections[section].push(cells);
        }
    }

    if sections[0].len() != HEIGHT || sections[1].len() != HEIGHT {
        return Err(format!(
            "需要解析到双方各 {HEIGHT} 行、每行 {WIDTH} 列；当前我方 {} 行，敌方 {} 行",
            sections[0].len(),
            sections[1].len()
        ));
    }

    Ok((to_grid(&sections[0]), to_grid(&sections[1])))
}

fn parse_row(line: &str, names: &[&str]) -> Vec<String> {
    let mut s = line.replace('\t', " ");
    if let Some(pos) = s.find('行') {
        s = s[pos + '行'.len_utf8()..].trim().to_string();
    }
    let tokens: Vec<String> = s
        .split_whitespace()
        .map(|x| x.trim_matches('|').to_string())
        .filter(|x| !x.is_empty())
        .collect();
    let mut found: Vec<String> = tokens
        .into_iter()
        .filter_map(|t| normalize_label(&t, names))
        .collect();
    if found.len() == WIDTH {
        return found;
    }
    found.clear();
    let mut rest = s.as_str();
    while !rest.trim().is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '|' || c == '\t');
        let mut matched = None;
        for name in names {
            if rest.starts_with(name) {
                matched = Some(*name);
                break;
            }
        }
        if let Some(name) = matched {
            let after_name = &rest[name.len()..];
            let digit_len = after_name
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .map(char::len_utf8)
                .sum::<usize>();
            let suffix = &after_name[..digit_len];
            found.push(format!("{name}{suffix}"));
            rest = &after_name[digit_len..];
        } else if let Some(ch) = rest.chars().next() {
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    found
}

fn normalize_label(token: &str, names: &[&str]) -> Option<String> {
    if names.contains(&token) {
        return Some(token.to_string());
    }
    for name in names {
        let Some(rest) = token.strip_prefix(name) else {
            continue;
        };
        if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(token.to_string());
        }
    }
    None
}

fn to_grid(rows: &[Vec<String>]) -> [[String; WIDTH]; HEIGHT] {
    std::array::from_fn(|r| std::array::from_fn(|c| rows[r][c].clone()))
}

fn build_items(grid: &[[String; WIDTH]; HEIGHT]) -> Result<Vec<Item>, String> {
    let all_defs = defs();
    let mut by_name = HashMap::new();
    for def in &all_defs {
        by_name.insert(def.short, *def);
    }
    let mut seen = [[false; WIDTH]; HEIGHT];
    let mut out = Vec::new();

    for r in 0..HEIGHT {
        for c in 0..WIDTH {
            if seen[r][c] {
                continue;
            }
            let label = grid[r][c].as_str();
            let (name, star) = split_star_label(label, &by_name)?;
            let def = *by_name
                .get(name)
                .ok_or_else(|| format!("未知装备词条: {name}"))?;
            let star_profile = match star {
                Some(value) => Some(
                    equipment::star_profile(name, value)
                        .ok_or_else(|| format!("{name} 暂未写入 {value} 星属性，无法按星级模拟"))?,
                ),
                None => None,
            };
            let comp = component(grid, &mut seen, r, c);
            let pieces = tile_component(&comp, def).ok_or_else(|| {
                format!(
                    "无法把 {name} 的 {} 个格子切成 {}*{} 或旋转形状",
                    comp.len(),
                    def.w,
                    def.h
                )
            })?;
            for cells in pieces {
                out.push(Item {
                    def,
                    _star: star,
                    star_profile,
                    cells,
                    attack_bonus: 0.0,
                    progress: 0.0,
                    frozen_until: 0.0,
                    accelerated_until: 0.0,
                    slowed_until: 0.0,
                    stacks: HashMap::new(),
                    used: false,
                });
            }
        }
    }
    Ok(out)
}

fn split_star_label<'a>(
    label: &'a str,
    by_name: &HashMap<&'static str, Def>,
) -> Result<(&'a str, Option<u8>), String> {
    if by_name.contains_key(label) {
        return Ok((label, None));
    }
    for name in by_name.keys() {
        let name = *name;
        let Some(rest) = label.strip_prefix(name) else {
            continue;
        };
        if rest.is_empty() || !rest.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let star = rest
            .parse::<u8>()
            .map_err(|_| format!("{label} 的星级数字无效"))?;
        if !(1..=4).contains(&star) {
            return Err(format!("{label} 的星级必须是 1/2/3/4"));
        }
        return Ok((name, Some(star)));
    }
    Err(format!("未知装备词条: {label}"))
}

fn component(
    grid: &[[String; WIDTH]; HEIGHT],
    seen: &mut [[bool; WIDTH]; HEIGHT],
    r: usize,
    c: usize,
) -> Vec<(usize, usize)> {
    let name = grid[r][c].clone();
    let mut q = VecDeque::from([(r, c)]);
    seen[r][c] = true;
    let mut out = Vec::new();
    while let Some((rr, cc)) = q.pop_front() {
        out.push((rr, cc));
        for (nr, nc) in neighbors(rr, cc) {
            if !seen[nr][nc] && grid[nr][nc] == name {
                seen[nr][nc] = true;
                q.push_back((nr, nc));
            }
        }
    }
    out
}

fn neighbors(r: usize, c: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    if r > 0 {
        v.push((r - 1, c));
    }
    if r + 1 < HEIGHT {
        v.push((r + 1, c));
    }
    if c > 0 {
        v.push((r, c - 1));
    }
    if c + 1 < WIDTH {
        v.push((r, c + 1));
    }
    v
}

fn tile_component(comp: &[(usize, usize)], def: Def) -> Option<Vec<Vec<(usize, usize)>>> {
    if def.w == 1 && def.h == 1 {
        return Some(comp.iter().map(|&cell| vec![cell]).collect());
    }
    let mut cells = comp.to_vec();
    cells.sort();
    let shapes = if def.w == def.h {
        vec![(def.h, def.w)]
    } else {
        vec![(def.h, def.w), (def.w, def.h)]
    };
    backtrack_tile(&cells, &shapes)
}

fn backtrack_tile(
    remaining: &[(usize, usize)],
    shapes: &[(usize, usize)],
) -> Option<Vec<Vec<(usize, usize)>>> {
    if remaining.is_empty() {
        return Some(Vec::new());
    }
    let &(r, c) = remaining.iter().min().unwrap();
    for &(h, w) in shapes {
        let mut piece = Vec::new();
        let mut ok = true;
        for rr in r..r + h {
            for cc in c..c + w {
                if remaining.contains(&(rr, cc)) {
                    piece.push((rr, cc));
                } else {
                    ok = false;
                }
            }
        }
        if !ok {
            continue;
        }
        let rest: Vec<(usize, usize)> = remaining
            .iter()
            .copied()
            .filter(|x| !piece.contains(x))
            .collect();
        if let Some(mut result) = backtrack_tile(&rest, shapes) {
            result.push(piece);
            return Some(result);
        }
    }
    None
}

fn adjacent(a: &[(usize, usize)], b: &[(usize, usize)]) -> bool {
    for &(ar, ac) in a {
        for &(br, bc) in b {
            if ar == br && ac.abs_diff(bc) == 1 {
                return true;
            }
            if ac == bc && ar.abs_diff(br) == 1 {
                return true;
            }
        }
    }
    false
}

fn sample_input() -> &'static str {
    r#"
现在用我这套：

行 / 列	第1列	第2列	第3列	第4列
第1行	百战	冰魄	通达	冰魄
第2行	吐纳	冰魄	战神	冰魄
第3行	玄武	冰魄	快意	冰魄
第4行	天响	穿颅	冰髓	冰髓
第5行	冻结	续命术	冰髓	冰髓

敌方用这套：

行 / 列	第1列	第2列	第3列	第4列
第1行	振魄	冰髓	冰髓	白虎之力
第2行	快意	冰髓	冰髓	穿颅
第3行	冰魄 通达 龙弧 龙弧
第4行	冰魄 通达 龙弧 龙弧
第5行	冰魄 烛神令 烛神令 烛神令
"#
}
