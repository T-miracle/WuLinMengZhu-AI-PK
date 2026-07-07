use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug)]
struct Args {
    query: String,
    detail: bool,
    db_path: PathBuf,
}

#[derive(Debug)]
struct Equipment {
    short: String,
    full: String,
    kind_label: String,
    bag_slots: String,
    attack: f64,
    interval: f64,
    tags: String,
}

#[derive(Debug)]
struct StarProfile {
    star: i64,
    attack: f64,
    interval: f64,
    normal_sword: i64,
}

#[derive(Clone, Debug)]
struct ActionRow {
    star: i64,
    trigger: String,
    action: String,
    target: String,
    amount: Option<f64>,
    duration: Option<f64>,
    count: Option<i64>,
    source_text: String,
    sort_order: i64,
    effects: BTreeMap<String, f64>,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ActionKey {
    trigger: String,
    action: String,
    target: String,
    sort_order: i64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(env::args().skip(1))?;
    let conn = Connection::open(&args.db_path)
        .map_err(|err| format!("打开数据库 {} 失败: {err}", args.db_path.display()))?;

    let matches = find_equipment(&conn, &args.query)?;
    match matches.len() {
        0 => {
            println!("未找到装备：{}", args.query);
            print_similar(&conn, &args.query)?;
        }
        1 => {
            print_equipment(&conn, &matches[0], args.detail)?;
        }
        _ => {
            println!("找到多件装备，请用更完整的名称或缩写查询：");
            for item in matches {
                println!("- {}（缩写：{}，类型：{}）", item.full, item.short, item.kind_label);
            }
        }
    }

    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut query_parts = Vec::new();
    let mut detail = false;
    let mut db_path = PathBuf::from("equipment.sqlite");
    let mut iter = args.peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--detail" | "-d" => detail = true,
            "--db" => {
                let Some(path) = iter.next() else {
                    return Err("--db 需要数据库路径".to_string());
                };
                db_path = PathBuf::from(path);
            }
            value if value.starts_with("--db=") => {
                db_path = PathBuf::from(value.trim_start_matches("--db="));
            }
            value if value.starts_with('-') => {
                return Err(format!("未知参数：{value}"));
            }
            value => query_parts.push(value.to_string()),
        }
    }

    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        print_help();
        return Err("请提供装备名称或缩写".to_string());
    }

    Ok(Args {
        query,
        detail,
        db_path,
    })
}

fn print_help() {
    println!("用法:");
    println!("  cargo run --bin equipment_query -- <装备名称或缩写>");
    println!("  cargo run --bin equipment_query -- --detail <装备名称或缩写>");
    println!("  cargo run --bin equipment_query -- --db equipment.sqlite <装备名称或缩写>");
    println!();
    println!("示例:");
    println!("  cargo run --bin equipment_query -- 通达");
    println!("  cargo run --bin equipment_query -- 暗锋魂玉");
}

fn find_equipment(conn: &Connection, query: &str) -> Result<Vec<Equipment>, String> {
    if let Some(item) = query_exact(conn, query)? {
        return Ok(vec![item]);
    }

    let pattern = format!("%{}%", query);
    let mut stmt = conn
        .prepare(
            "SELECT short, full, kind_label, bag_slots, attack, interval,
                    attribute_tags_text
             FROM equipment
             WHERE short LIKE ?1 OR full LIKE ?1
             ORDER BY sort_order, short",
        )
        .map_err(|err| format!("准备查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![pattern], read_equipment)
        .map_err(|err| format!("查询装备失败: {err}"))?;

    collect_rows(rows)
}

fn query_exact(conn: &Connection, query: &str) -> Result<Option<Equipment>, String> {
    conn.query_row(
        "SELECT short, full, kind_label, bag_slots, attack, interval,
                attribute_tags_text
         FROM equipment
         WHERE short = ?1 OR full = ?1
         ORDER BY CASE WHEN short = ?1 THEN 0 ELSE 1 END
         LIMIT 1",
        params![query],
        read_equipment,
    )
    .optional()
    .map_err(|err| format!("查询装备失败: {err}"))
}

fn read_equipment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Equipment> {
    Ok(Equipment {
        short: row.get(0)?,
        full: row.get(1)?,
        kind_label: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        bag_slots: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        attack: row.get(4)?,
        interval: row.get(5)?,
        tags: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("解析查询结果失败: {err}"))?);
    }
    Ok(out)
}

fn print_equipment(conn: &Connection, item: &Equipment, detail: bool) -> Result<(), String> {
    let star_profiles = load_star_profiles(conn, &item.short)?;
    let actions = load_actions(conn, &item.short)?;

    println!("名称：{}", item.full);
    println!("属性：{}", item.kind_label);
    if detail && let Some(multi_trigger) = multi_trigger_label(&actions) {
        println!("多重触发：{multi_trigger}");
    }
    if !detail {
        let summary = generated_properties(item, &star_profiles, &actions);
        if summary != "暂无可由结构化表生成的属性信息" {
            println!("{}", summary);
        }
    }

    if detail {
        println!();
        println!("缩写：{}", item.short);
        println!("背包占用：{}", item.bag_slots);
        println!("关联附属属性：{}", empty_as_dash(&item.tags));

        if !star_profiles.is_empty() {
            println!();
            println!("星级档位：");
            for profile in star_profiles {
                println!("- {}星：{}", profile.star, star_detail_text(&profile, &actions));
            }
        } else {
            println!();
            println!("属性详情：{}", generated_properties(item, &[], &actions));
        }
    }

    Ok(())
}

fn load_star_profiles(conn: &Connection, short: &str) -> Result<Vec<StarProfile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT star, attack, interval, normal_sword
             FROM equipment_star_profiles
             WHERE short = ?1
             ORDER BY star",
        )
        .map_err(|err| format!("准备星级查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![short], |row| {
            Ok(StarProfile {
                star: row.get(0)?,
                attack: row.get(1)?,
                interval: row.get(2)?,
                normal_sword: row.get(3)?,
            })
        })
        .map_err(|err| format!("查询星级档位失败: {err}"))?;

    collect_rows(rows)
}

fn load_actions(conn: &Connection, short: &str) -> Result<Vec<ActionRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT star, trigger, action, target, amount, duration, count, source_text, sort_order
             FROM equipment_actions
             WHERE equipment_short = ?1
             ORDER BY star, sort_order, trigger, action",
        )
        .map_err(|err| format!("准备行为查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![short], |row| {
            Ok(ActionRow {
                star: row.get(0)?,
                trigger: row.get(1)?,
                action: row.get(2)?,
                target: row.get(3)?,
                amount: row.get(4)?,
                duration: row.get(5)?,
                count: row.get(6)?,
                source_text: row.get(7)?,
                sort_order: row.get(8)?,
                effects: BTreeMap::new(),
            })
        })
        .map_err(|err| format!("查询装备行为失败: {err}"))?;

    let mut actions = collect_rows(rows)?;
    for action in &mut actions {
        action.effects = load_action_effects(conn, short, action)?;
    }
    Ok(actions)
}

fn load_action_effects(
    conn: &Connection,
    short: &str,
    action: &ActionRow,
) -> Result<BTreeMap<String, f64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT effect_key, value
             FROM equipment_action_effects
             WHERE equipment_short = ?1
               AND star = ?2
               AND trigger = ?3
               AND action = ?4
               AND target = ?5
               AND sort_order = ?6
             ORDER BY effect_key",
        )
        .map_err(|err| format!("准备动作效果查询失败: {err}"))?;
    let rows = stmt
        .query_map(
            params![
                short,
                action.star,
                action.trigger,
                action.action,
                action.target,
                action.sort_order
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
        )
        .map_err(|err| format!("查询动作效果失败: {err}"))?;

    let mut out = BTreeMap::new();
    for row in rows {
        let (key, value) = row.map_err(|err| format!("读取动作效果失败: {err}"))?;
        out.insert(key, value);
    }
    Ok(out)
}

fn multi_trigger_label(actions: &[ActionRow]) -> Option<String> {
    let mut values = actions
        .iter()
        .filter(|action| action.trigger == "active_use")
        .filter_map(|action| action.effects.get("multi_trigger").copied().or(action.amount))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.total_cmp(right));
    values.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    values
        .first()
        .copied()
        .map(format_number)
        .filter(|value| value != "-")
}

fn star_detail_text(profile: &StarProfile, actions: &[ActionRow]) -> String {
    let mut parts = Vec::new();
    if profile.attack != 0.0 {
        parts.push(format!("攻击力 {}", format_number(profile.attack)));
    }
    if profile.interval != 0.0 {
        parts.push(format!("发动间隔 {}", format_number(profile.interval)));
    }
    if profile.normal_sword != 0 {
        parts.push(format!("普攻剑势 {}", profile.normal_sword));
    }

    for action in actions_for_star(actions, profile.star) {
        match (action.trigger.as_str(), action.action.as_str()) {
            ("active_use", "freezing_normal_weapon_attack") => {
                parts.push(format!(
                    "冻结对手随机{}件道具{}秒",
                    effect_label(&action, "freeze_count", action.count.map(|value| value as f64)),
                    effect_label(&action, "freeze_duration", action.duration)
                ));
            }
            ("active_use", "charged_healing_weapon_attack") => {
                parts.push(format!(
                    "蓄力攻击消耗{}点剑势，造成{}倍武器攻击力的伤害，生命恢复+{}点",
                    effect_label(&action, "charged_sword_cost", None),
                    effect_label(&action, "charged_damage_multiplier", None),
                    effect_label(&action, "charged_heal", None)
                ));
                parts.push(format!(
                    "普通攻击造成等同于武器攻击力的伤害，剑势+{}点，生命恢复+{}点",
                    effect_label(&action, "normal_sword", Some(profile.normal_sword as f64)),
                    effect_label(&action, "normal_heal", None)
                ));
            }
            ("active_use", "charged_sword_control_weapon_attack") => {
                parts.push(format!(
                    "蓄力攻击消耗{}点剑势，造成{}倍武器攻击力的伤害，对手剑势-{}点，冻结对手随机{}件道具{}秒",
                    effect_label(&action, "charged_sword_cost", None),
                    effect_label(&action, "charged_damage_multiplier", None),
                    effect_label(&action, "charged_enemy_sword_loss", None),
                    effect_label(&action, "freeze_count", action.count.map(|value| value as f64)),
                    effect_label(&action, "freeze_duration", action.duration)
                ));
            }
            ("on_freeze_source", "add_freeze_attack_stack") => {
                parts.push(format!(
                    "触发冻结所有武器攻击加成 {}，最多叠加{}次",
                    effect_label(&action, "attack_bonus", action.amount),
                    effect_label(&action, "max_stacks", action.count.map(|value| value as f64))
                ));
            }
            ("battle_start", "start_sword") => {
                parts.push(format!(
                    "战斗初始剑势+{}",
                    effect_label(&action, "start_sword", action.amount)
                ));
            }
            ("on_charged_hit", "sword") => {
                parts.push(format!(
                    "蓄力命中剑势+{}",
                    action
                        .amount
                        .map(format_number)
                        .unwrap_or_else(|| "-".to_string())
                ));
            }
            ("on_charged_hit", "freeze_random") => {
                parts.push(format!(
                    "蓄力命中冻结对手随机{}件道具{}秒",
                    action.count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
                    action.duration.map(format_number).unwrap_or_else(|| "-".to_string())
                ));
            }
            ("on_attack_hit", "enemy_sword") => {
                parts.push(format!(
                    "攻击命中对手剑势-{}",
                    effect_label(&action, "enemy_sword_loss", action.amount)
                ));
            }
            ("on_heal", "charge_self") => {
                parts.push(format!(
                    "受到治疗后本武器充能{}秒",
                    effect_label(&action, "charge_seconds", action.amount)
                ));
            }
            ("on_crit", "slow_random") => {
                parts.push(format!(
                    "暴击减速对手随机{}件道具{}秒",
                    action.count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
                    action.duration.map(format_number).unwrap_or_else(|| "-".to_string())
                ));
            }
            ("on_accelerated", "add_adjacent_weapon_attack_stacking_if_adjacent_to_context") => {
                parts.push(format!(
                    "我方道具被加速时相邻武器攻击加成 {}，最多叠加{}层",
                    action
                        .amount
                        .map(format_number)
                        .unwrap_or_else(|| "-".to_string()),
                    action.count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string())
                ));
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        "暂无可由结构化表生成的星级信息".to_string()
    } else {
        parts.join("，")
    }
}

fn actions_for_star(actions: &[ActionRow], star: i64) -> Vec<ActionRow> {
    let mut merged: BTreeMap<ActionKey, (i64, ActionRow)> = BTreeMap::new();
    for action in actions {
        if action.star != 0 && action.star != star {
            continue;
        }
        let key = action_key(action);
        let replace = merged
            .get(&key)
            .is_none_or(|(existing_star, _)| action.star >= *existing_star);
        if replace {
            merged.insert(key, (action.star, action.clone()));
        }
    }
    merged.into_values().map(|(_, action)| action).collect()
}

fn effect_label(action: &ActionRow, key: &str, fallback: Option<f64>) -> String {
    action
        .effects
        .get(key)
        .copied()
        .or(fallback)
        .map(format_number)
        .unwrap_or_else(|| "-".to_string())
}

fn generated_properties(
    item: &Equipment,
    star_profiles: &[StarProfile],
    actions: &[ActionRow],
) -> String {
    let mut parts = Vec::new();

    let mut action_groups: BTreeMap<ActionKey, Vec<&ActionRow>> = BTreeMap::new();
    for action in actions {
        action_groups.entry(action_key(action)).or_default().push(action);
    }
    let mut groups = action_groups.values().collect::<Vec<_>>();
    groups.sort_by_key(|group| {
        group
            .iter()
            .map(|action| action.sort_order)
            .min()
            .unwrap_or_default()
    });
    for group in groups {
        if let Some(text) = action_group_text(group, star_profiles) {
            if !parts.contains(&text) {
                parts.push(text);
            }
        }
    }

    if parts.is_empty() {
        fallback_properties(item)
    } else {
        combine_related_parts(parts).join("；")
    }
}

fn combine_related_parts(parts: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < parts.len() {
        if idx + 1 < parts.len()
            && parts[idx].starts_with("蓄力攻击命中后：剑势+")
            && parts[idx + 1].starts_with("蓄力攻击命中后：冻结")
        {
            let freeze = parts[idx + 1].trim_start_matches("蓄力攻击命中后：");
            out.push(format!("{}，并{}", parts[idx], freeze));
            idx += 2;
        } else {
            out.push(parts[idx].clone());
            idx += 1;
        }
    }
    out
}

fn action_group_text(actions: &[&ActionRow], star_profiles: &[StarProfile]) -> Option<String> {
    let first = actions.first()?;
    match (first.trigger.as_str(), first.action.as_str()) {
        ("battle_start", "start_sword") => Some(format!(
            "战斗初始剑势+{}点",
            action_values_label(actions, ValueField::Amount)
        )),
        ("battle_start", "accelerate_all_active") => Some(format!(
            "战斗初始所有道具加速{}秒",
            action_values_label(actions, ValueField::Duration)
        )),
        ("on_normal_hit", "add_weapon_attack_stacking") => Some(format!(
            "普通攻击命中后：所有武器+{}点攻击力，最多叠加{}次",
            action_values_label(actions, ValueField::Amount),
            action_values_label(actions, ValueField::Count)
        )),
        ("on_attack_hit", "enemy_sword") => Some(format!(
            "攻击命中时：对手剑势-{}点",
            action_values_label(actions, ValueField::Amount)
        )),
        ("on_charged_hit", "sword") => Some(format!(
            "蓄力攻击命中后：剑势+{}点",
            action_values_label(actions, ValueField::Amount)
        )),
        ("on_charged_hit", "freeze_random") => Some(format!(
            "蓄力攻击命中后：冻结对手随机{}件道具{}秒",
            action_values_label(actions, ValueField::Count),
            action_values_label(actions, ValueField::Duration)
        )),
        ("on_crit", "slow_random") => Some(format!(
            "暴击时：减速对手随机{}件道具{}秒",
            action_values_label(actions, ValueField::Count),
            action_values_label(actions, ValueField::Duration)
        )),
        ("on_accelerated", "add_adjacent_weapon_attack_stacking_if_adjacent_to_context") => {
            Some(format!(
                "我方道具被加速时：相邻武器+{}点攻击力，最多叠加{}层",
            action_values_label(actions, ValueField::Amount),
            action_values_label(actions, ValueField::Count)
            ))
        }
        ("on_freeze_source", "add_freeze_attack_stack") => Some(format!(
            "每次触发冻结时：所有武器+{}点攻击力，最多叠加{}次",
            action_values_label(actions, ValueField::Amount),
            action_values_label(actions, ValueField::Count)
        )),
        ("active_use", action) => Some(active_use_text(action, first, actions, star_profiles)),
        _ => Some(format!(
            "{}：{} -> {}{}{}{}（来源：{}）",
            trigger_label(&first.trigger),
            first.action,
            target_label(&first.target),
            optional_f64_text("，数值", first.amount),
            optional_f64_text("，持续", first.duration),
            optional_i64_text("，次数", first.count),
            first.source_text
        )),
    }
}

fn action_key(action: &ActionRow) -> ActionKey {
    ActionKey {
        trigger: action.trigger.clone(),
        action: action.action.clone(),
        target: action.target.clone(),
        sort_order: action.sort_order,
    }
}

#[derive(Clone, Copy)]
enum ValueField {
    Amount,
    Duration,
    Count,
}

fn action_values_label(actions: &[&ActionRow], field: ValueField) -> String {
    let mut starred = actions
        .iter()
        .filter(|action| (1..=4).contains(&action.star))
        .collect::<Vec<_>>();
    starred.sort_by_key(|action| action.star);
    if starred.len() == 4 {
        let values = starred
            .into_iter()
            .map(|action| action_value(action, field))
            .collect::<Vec<_>>();
        return collapse_values(values);
    }

    actions
        .iter()
        .copied()
        .find(|action| action.star == 0)
        .or_else(|| actions.first().copied())
        .map(|action| action_value(action, field))
        .unwrap_or_else(|| "-".to_string())
}

fn action_value(action: &ActionRow, field: ValueField) -> String {
    match field {
        ValueField::Amount => action.amount.map(format_number).unwrap_or_else(|| "-".to_string()),
        ValueField::Duration => action
            .duration
            .map(format_number)
            .unwrap_or_else(|| "-".to_string()),
        ValueField::Count => action
            .count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
    }
}

fn collapse_values(values: Vec<String>) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    if values.windows(2).all(|pair| pair[0] == pair[1]) {
        values[0].clone()
    } else {
        values.join("/")
    }
}

fn active_use_text(
    action: &str,
    row: &ActionRow,
    actions: &[&ActionRow],
    star_profiles: &[StarProfile],
) -> String {
    match action {
        "freezing_normal_weapon_attack" => format!(
            "多重触发{}；普通攻击：造成等同于武器攻击力的伤害，剑势+{}点，冻结对手随机{}件道具{}秒",
            action_values_label(actions, ValueField::Amount),
            normal_sword_label(star_profiles),
            action_values_label(actions, ValueField::Count),
            action_values_label(actions, ValueField::Duration)
        ),
        "charged_sword_control_weapon_attack" => "蓄力攻击：消耗剑势造成高额伤害，对手剑势降低，并冻结对手随机道具；剑势不足时发动普通攻击".to_string(),
        "charged_healing_weapon_attack" => "蓄力攻击：消耗剑势造成高额伤害并恢复生命；剑势不足时发动普通攻击".to_string(),
        "freeze_random" => format!(
            "发动效果：冻结对手随机{}件道具{}秒",
            row.count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            row.duration.map(format_number).unwrap_or_else(|| "-".to_string())
        ),
        "heal" => format!(
            "发动效果：治疗{}点生命值",
            row.amount.map(format_number).unwrap_or_else(|| "-".to_string())
        ),
        "sword" => format!(
            "发动效果：剑势+{}点",
            row.amount.map(format_number).unwrap_or_else(|| "-".to_string())
        ),
        "charge_random" => format!(
            "发动效果：充能我方随机道具{}秒",
            row.amount.map(format_number).unwrap_or_else(|| "-".to_string())
        ),
        "mark_used" => "发动后标记为已使用".to_string(),
        other => format!("发动效果：{other}"),
    }
}

fn normal_sword_label(star_profiles: &[StarProfile]) -> String {
    if star_profiles.is_empty() {
        return "-".to_string();
    }
    collapse_values(
        star_profiles
            .iter()
            .map(|profile| profile.normal_sword.to_string())
            .collect(),
    )
}

fn fallback_properties(item: &Equipment) -> String {
    let mut parts = Vec::new();
    if item.attack != 0.0 {
        parts.push(format!("攻击力{}", format_number(item.attack)));
    }
    if item.interval != 0.0 {
        parts.push(format!("发动间隔{}", format_number(item.interval)));
    }
    if parts.is_empty() {
        "暂无可由结构化表生成的属性信息".to_string()
    } else {
        parts.join("；")
    }
}

fn trigger_label(trigger: &str) -> &str {
    match trigger {
        "battle_start" => "战斗初始",
        "active_use" => "发动效果",
        "on_attack_hit" => "攻击命中时",
        "on_normal_hit" => "普通攻击命中后",
        "on_charged_hit" => "蓄力攻击命中后",
        "on_crit" => "暴击时",
        "on_accelerated" => "我方道具被加速时",
        "on_heal" => "受到治疗后",
        "on_freeze_triggered" => "冻结触发时",
        other => other,
    }
}

fn target_label(target: &str) -> &str {
    match target {
        "self" => "自身",
        "enemy" => "对手",
        "self_weapons" => "所有武器",
        "self_adjacent_weapons" => "相邻武器",
        "enemy_active" => "对手读条道具",
        "self_active" => "我方读条道具",
        other => other,
    }
}

fn optional_f64_text(label: &str, value: Option<f64>) -> String {
    value
        .map(|value| format!("{label}{}", format_number(value)))
        .unwrap_or_default()
}

fn optional_i64_text(label: &str, value: Option<i64>) -> String {
    value
        .map(|value| format!("{label}{value}"))
        .unwrap_or_default()
}

fn print_similar(conn: &Connection, query: &str) -> Result<(), String> {
    let Some(first_char) = query.chars().next() else {
        return Ok(());
    };
    let pattern = format!("%{}%", first_char);
    let mut stmt = conn
        .prepare(
            "SELECT short, full, kind_label, bag_slots, attack, interval,
                    attribute_tags_text
             FROM equipment
             WHERE short LIKE ?1 OR full LIKE ?1
             ORDER BY sort_order, short
             LIMIT 8",
        )
        .map_err(|err| format!("准备相似查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![pattern], read_equipment)
        .map_err(|err| format!("查询相似装备失败: {err}"))?;
    let similar = collect_rows(rows)?;

    if !similar.is_empty() {
        println!("相近候选：");
        for item in similar {
            println!("- {}（缩写：{}）", item.full, item.short);
        }
    }
    Ok(())
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < 1e-9 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}
