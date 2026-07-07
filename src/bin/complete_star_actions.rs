use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use rusqlite::{Connection, params};

#[derive(Clone, Debug)]
struct Args {
    db_path: PathBuf,
}

#[derive(Clone, Debug)]
struct Equipment {
    short: String,
    full: String,
    properties: String,
}

#[derive(Clone, Debug)]
struct Action {
    star: i64,
    trigger: String,
    action: String,
    target: String,
    amount: Option<f64>,
    duration: Option<f64>,
    count: Option<i64>,
    source_text: String,
    sort_order: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
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
    let mut conn = Connection::open(&args.db_path)
        .map_err(|err| format!("打开数据库 {} 失败: {err}", args.db_path.display()))?;
    ensure_equipment_actions_schema(&conn)?;

    let items = load_star_items(&conn)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启数据库事务失败: {err}"))?;

    let mut changed = 0;
    for item in items {
        let templates = load_generic_actions(&tx, &item.short)?;
        let inferred_by_star = infer_star_actions(&item);
        if templates.is_empty() && inferred_by_star.values().all(Vec::is_empty) {
            println!("- {}：有星级描述，但暂未匹配到可写入动作", item.full);
            continue;
        }

        tx.execute(
            "DELETE FROM equipment_actions WHERE equipment_short = ?1 AND star BETWEEN 1 AND 4",
            params![item.short],
        )
        .map_err(|err| format!("清理 {} 星级动作失败: {err}", item.full))?;

        let mut inserted = 0;
        for star in 1..=4 {
            let mut actions = templates
                .iter()
                .cloned()
                .map(|mut action| {
                    action.star = star;
                    action
                })
                .collect::<Vec<_>>();
            merge_inferred_actions(&mut actions, inferred_by_star.get(&star).cloned().unwrap_or_default());
            for action in actions {
                insert_action(&tx, &item.short, &action)?;
                inserted += 1;
            }
        }
        changed += 1;
        println!("- {}：补齐 {} 条星级动作", item.full, inserted);
    }

    tx.commit()
        .map_err(|err| format!("提交数据库事务失败: {err}"))?;
    println!("完成：处理 {} 件存在星级区分的装备。", changed);
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut db_path = PathBuf::from("equipment.sqlite");
    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("用法:");
                println!("  cargo run --bin complete_star_actions");
                println!("  cargo run --bin complete_star_actions -- --db equipment.sqlite");
                std::process::exit(0);
            }
            "--db" => {
                db_path = iter
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| "--db 需要数据库路径".to_string())?;
            }
            value if value.starts_with("--db=") => {
                db_path = PathBuf::from(value.trim_start_matches("--db="));
            }
            value => return Err(format!("未知参数：{value}")),
        }
    }
    Ok(Args { db_path })
}

fn load_star_items(conn: &Connection) -> Result<Vec<Equipment>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT short, full, COALESCE(properties_text, '')
             FROM equipment
             ORDER BY sort_order, short",
        )
        .map_err(|err| format!("准备装备查询失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Equipment {
                short: row.get(0)?,
                full: row.get(1)?,
                properties: row.get(2)?,
            })
        })
        .map_err(|err| format!("查询装备失败: {err}"))?;

    let mut out = Vec::new();
    for row in rows {
        let item = row.map_err(|err| format!("读取装备失败: {err}"))?;
        if has_star_values(&item.properties) {
            out.push(item);
        }
    }
    Ok(out)
}

fn load_generic_actions(conn: &Connection, short: &str) -> Result<Vec<Action>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT trigger, action, target, amount, duration, count, source_text, sort_order
             FROM equipment_actions
             WHERE equipment_short = ?1 AND star = 0
             ORDER BY sort_order",
        )
        .map_err(|err| format!("准备通用动作查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![short], |row| {
            Ok(Action {
                star: 0,
                trigger: row.get(0)?,
                action: row.get(1)?,
                target: row.get(2)?,
                amount: row.get(3)?,
                duration: row.get(4)?,
                count: row.get(5)?,
                source_text: row.get(6)?,
                sort_order: row.get(7)?,
            })
        })
        .map_err(|err| format!("查询通用动作失败: {err}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("读取通用动作失败: {err}"))?);
    }
    Ok(out)
}

fn infer_star_actions(item: &Equipment) -> BTreeMap<i64, Vec<Action>> {
    (1..=4)
        .map(|star| {
            let properties = properties_for_star(&item.properties, star as usize);
            (star, infer_actions(&item.short, &item.full, &properties, star))
        })
        .collect()
}

fn merge_inferred_actions(actions: &mut Vec<Action>, inferred: Vec<Action>) {
    for inferred_action in inferred {
        let key = action_key(&inferred_action);
        if let Some(existing) = actions.iter_mut().find(|action| action_key(action) == key) {
            existing.amount = inferred_action.amount;
            existing.duration = inferred_action.duration;
            existing.count = inferred_action.count;
            existing.source_text = inferred_action.source_text;
        } else {
            actions.push(inferred_action);
        }
    }
    actions.sort_by_key(|action| (action.star, action.sort_order, action.trigger.clone()));
}

fn action_key(action: &Action) -> ActionKey {
    ActionKey {
        trigger: action.trigger.clone(),
        action: action.action.clone(),
        target: action.target.clone(),
        sort_order: action.sort_order,
    }
}

fn insert_action(conn: &Connection, short: &str, action: &Action) -> Result<(), String> {
    conn.execute(
        "INSERT INTO equipment_actions
         (equipment_short, star, trigger, action, target, amount, duration, count, source_text, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            short,
            action.star,
            action.trigger,
            action.action,
            action.target,
            action.amount,
            action.duration,
            action.count,
            action.source_text,
            action.sort_order,
        ],
    )
    .map_err(|err| format!("写入 {short} 星级动作失败: {err}"))?;
    Ok(())
}

fn infer_actions(short: &str, full: &str, properties: &str, star: i64) -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(normal_text) = section_after(properties, "普通攻击") {
        if normal_text.contains("冻结对手随机")
            && normal_text.contains("道具")
            && normal_text.contains("秒")
        {
        actions.push(Action {
            star,
            trigger: "active_use".to_string(),
            action: "freezing_normal_weapon_attack".to_string(),
            target: "enemy".to_string(),
            amount: number_between(properties, "多重触发", "；").or(Some(1.0)),
            duration: number_between(normal_text, "道具", "秒"),
            count: number_between(normal_text, "冻结对手随机", "件").map(|value| value as i64),
            source_text: short.to_string(),
            sort_order: 10,
        });
        }
    }

    if properties.contains("普通攻击命中")
        && properties.contains("所有武器+")
        && properties.contains("攻击力")
    {
        actions.push(Action {
            star,
            trigger: "on_normal_hit".to_string(),
            action: "add_weapon_attack_stacking".to_string(),
            target: "self_weapons".to_string(),
            amount: number_between(properties, "所有武器+", "攻击力"),
            duration: None,
            count: number_between(properties, "最多叠加", "次").map(|value| value as i64),
            source_text: full.to_string(),
            sort_order: 10,
        });
    }

    if properties.contains("触发冻结")
        && properties.contains("所有武器+")
        && properties.contains("攻击力")
    {
        actions.push(Action {
            star,
            trigger: "on_freeze_source".to_string(),
            action: "add_freeze_attack_stack".to_string(),
            target: "self_weapons".to_string(),
            amount: number_between(properties, "所有武器+", "点")
                .or_else(|| number_between(properties, "所有武器+", "攻击力")),
            duration: None,
            count: number_between(properties, "最多叠加", "次").map(|value| value as i64),
            source_text: short.to_string(),
            sort_order: 10,
        });
    }

    if properties.contains("攻击命中") && properties.contains("对手剑势-") {
        actions.push(Action {
            star,
            trigger: "on_attack_hit".to_string(),
            action: "enemy_sword".to_string(),
            target: "enemy".to_string(),
            amount: number_between(properties, "对手剑势-", "点"),
            duration: None,
            count: None,
            source_text: full.to_string(),
            sort_order: 10,
        });
    }

    if properties.contains("战斗初始剑势+") {
        actions.push(Action {
            star,
            trigger: "battle_start".to_string(),
            action: "start_sword".to_string(),
            target: "self".to_string(),
            amount: number_between(properties, "战斗初始剑势+", "点"),
            duration: None,
            count: None,
            source_text: full.to_string(),
            sort_order: 10,
        });
    }

    let hit_text = after(properties, "蓄力攻击命中后").or_else(|| after(properties, "命中后"));
    if let Some(hit_text) = hit_text {
        if hit_text.contains("剑势+") {
            actions.push(Action {
                star,
                trigger: "on_charged_hit".to_string(),
                action: "sword".to_string(),
                target: "self".to_string(),
                amount: number_between(hit_text, "剑势+", "点"),
                duration: None,
                count: None,
                source_text: short.to_string(),
                sort_order: 10,
            });
        }
        if hit_text.contains("冻结") {
            actions.push(Action {
                star,
                trigger: "on_charged_hit".to_string(),
                action: "freeze_random".to_string(),
                target: "enemy_active".to_string(),
                amount: None,
                duration: number_between(hit_text, "道具", "秒"),
                count: number_between(hit_text, "随机", "件").map(|value| value as i64),
                source_text: short.to_string(),
                sort_order: 20,
            });
        }
    }

    if properties.contains("暴击时") && properties.contains("减速") {
        actions.push(Action {
            star,
            trigger: "on_crit".to_string(),
            action: "slow_random".to_string(),
            target: "enemy_active".to_string(),
            amount: None,
            duration: number_between(properties, "道具", "秒")
                .or_else(|| number_between(properties, "减速", "秒")),
            count: number_between(properties, "随机", "件").map(|value| value as i64),
            source_text: full.to_string(),
            sort_order: 10,
        });
    }

    if properties.contains("战斗初始所有道具加速") {
        actions.push(Action {
            star,
            trigger: "battle_start".to_string(),
            action: "accelerate_all_active".to_string(),
            target: "self_active".to_string(),
            amount: None,
            duration: number_between(properties, "战斗初始所有道具加速", "秒"),
            count: None,
            source_text: full.to_string(),
            sort_order: 10,
        });
    }

    if properties.contains("我方道具被加速时") && properties.contains("相邻武器+") {
        actions.push(Action {
            star,
            trigger: "on_accelerated".to_string(),
            action: "add_adjacent_weapon_attack_stacking_if_adjacent_to_context".to_string(),
            target: "self_adjacent_weapons".to_string(),
            amount: number_between(properties, "相邻武器+", "攻击力"),
            duration: None,
            count: number_between(properties, "最多叠加", "层").map(|value| value as i64),
            source_text: full.to_string(),
            sort_order: 10,
        });
    }
    actions
}

fn has_star_values(text: &str) -> bool {
    slash_groups(text)
        .iter()
        .any(|group| group.len() == 4 && group.windows(2).any(|pair| pair[0] != pair[1]))
}

fn properties_for_star(text: &str, star: usize) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        if chars[idx].is_ascii_digit() || chars[idx] == '.' {
            let start = idx;
            let mut end = idx;
            while end < chars.len()
                && (chars[end].is_ascii_digit() || chars[end] == '.' || chars[end] == '/')
            {
                end += 1;
            }
            let token: String = chars[start..end].iter().collect();
            if let Some(values) = parse_slash_group(&token) {
                let value = values
                    .get(star.saturating_sub(1))
                    .copied()
                    .unwrap_or_else(|| *values.last().unwrap_or(&0.0));
                out.push_str(&format_number(value));
            } else {
                out.push_str(&token);
            }
            idx = end;
        } else {
            out.push(chars[idx]);
            idx += 1;
        }
    }
    out
}

fn slash_groups(text: &str) -> Vec<Vec<f64>> {
    let mut groups = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '/' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Some(group) = parse_slash_group(&current) {
                groups.push(group);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Some(group) = parse_slash_group(&current) {
            groups.push(group);
        }
    }
    groups
}

fn parse_slash_group(token: &str) -> Option<Vec<f64>> {
    if !token.contains('/') {
        return None;
    }
    let values: Option<Vec<f64>> = token.split('/').map(|part| part.parse().ok()).collect();
    values.filter(|values| !values.is_empty())
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    text.find(marker).map(|idx| &text[idx + marker.len()..])
}

fn section_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = after(text, marker)?;
    let stop = rest.find('；').unwrap_or(rest.len());
    Some(&rest[..stop])
}

fn number_between(text: &str, start: &str, end: &str) -> Option<f64> {
    let text = after(text, start)?;
    let stop = text.find(end).unwrap_or(text.len());
    max_number(&text[..stop])
}

fn max_number(text: &str) -> Option<f64> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(value) = current.parse::<f64>() {
            numbers.push(value);
        }
    }
    numbers.into_iter().reduce(f64::max)
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{}", value as i64)
    } else {
        format!("{value:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn ensure_equipment_actions_schema(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(equipment_actions)")
        .map_err(|err| format!("读取装备行为表结构失败: {err}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|err| format!("读取装备行为表结构失败: {err}"))?;
    let mut has_star = false;
    while let Some(row) = rows
        .next()
        .map_err(|err| format!("读取装备行为表结构失败: {err}"))?
    {
        let name: String = row
            .get(1)
            .map_err(|err| format!("读取装备行为表结构失败: {err}"))?;
        if name == "star" {
            has_star = true;
            break;
        }
    }
    drop(rows);
    drop(stmt);

    if has_star {
        return Ok(());
    }

    conn.execute_batch(
        "ALTER TABLE equipment_actions RENAME TO equipment_actions_old;
         CREATE TABLE equipment_actions (
            equipment_short TEXT NOT NULL,
            star INTEGER NOT NULL DEFAULT 0,
            trigger TEXT NOT NULL,
            action TEXT NOT NULL,
            target TEXT NOT NULL,
            amount REAL,
            duration REAL,
            count INTEGER,
            source_text TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (equipment_short, star, trigger, action, target, sort_order),
            FOREIGN KEY (equipment_short) REFERENCES equipment(short) ON DELETE CASCADE
         );
         INSERT OR IGNORE INTO equipment_actions
            (equipment_short, star, trigger, action, target, amount, duration, count, source_text, sort_order)
         SELECT equipment_short, 0, trigger, action, target, amount, duration, count, source_text, sort_order
         FROM equipment_actions_old;
         DROP TABLE equipment_actions_old;",
    )
    .map_err(|err| format!("升级装备行为表结构失败: {err}"))
}
