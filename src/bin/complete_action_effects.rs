use std::env;
use std::path::PathBuf;

use rusqlite::{Connection, params};

#[derive(Debug)]
struct Args {
    db_path: PathBuf,
}

#[derive(Debug)]
struct Equipment {
    short: String,
    properties: String,
}

#[derive(Debug)]
struct Action {
    star: i64,
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
    create_schema(&conn)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启数据库事务失败: {err}"))?;
    tx.execute("DELETE FROM equipment_action_effects", [])
        .map_err(|err| format!("清理动作效果参数失败: {err}"))?;

    let items = load_equipment(&tx)?;
    let mut inserted = 0;
    for item in items {
        let actions = load_actions(&tx, &item.short)?;
        for action in actions {
            let props = if (1..=4).contains(&action.star) {
                properties_for_star(&item.properties, action.star as usize)
            } else {
                properties_for_star(&item.properties, 4)
            };
            for (key, value, unit) in infer_effects(&props, &action) {
                insert_effect(&tx, &item.short, &action, key, value, unit)?;
                inserted += 1;
            }
        }
    }

    tx.commit()
        .map_err(|err| format!("提交动作效果参数失败: {err}"))?;
    println!("已写入 {inserted} 条动作效果参数。");
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut db_path = PathBuf::from("equipment.sqlite");
    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("用法:");
                println!("  cargo run --bin complete_action_effects");
                println!("  cargo run --bin complete_action_effects -- --db equipment.sqlite");
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

fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS equipment_action_effects (
            equipment_short TEXT NOT NULL,
            star INTEGER NOT NULL DEFAULT 0,
            trigger TEXT NOT NULL,
            action TEXT NOT NULL,
            target TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            effect_key TEXT NOT NULL,
            value REAL NOT NULL,
            unit TEXT,
            PRIMARY KEY (equipment_short, star, trigger, action, target, sort_order, effect_key),
            FOREIGN KEY (equipment_short) REFERENCES equipment(short) ON DELETE CASCADE
        );",
    )
    .map_err(|err| format!("初始化动作效果参数表失败: {err}"))
}

fn load_equipment(conn: &Connection) -> Result<Vec<Equipment>, String> {
    let mut stmt = conn
        .prepare("SELECT short, COALESCE(properties_text, '') FROM equipment ORDER BY sort_order")
        .map_err(|err| format!("准备装备查询失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Equipment {
                short: row.get(0)?,
                properties: row.get(1)?,
            })
        })
        .map_err(|err| format!("查询装备失败: {err}"))?;
    collect(rows, "读取装备")
}

fn load_actions(conn: &Connection, short: &str) -> Result<Vec<Action>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT star, trigger, action, target, sort_order
             FROM equipment_actions
             WHERE equipment_short = ?1
             ORDER BY star, sort_order",
        )
        .map_err(|err| format!("准备动作查询失败: {err}"))?;
    let rows = stmt
        .query_map(params![short], |row| {
            Ok(Action {
                star: row.get(0)?,
                trigger: row.get(1)?,
                action: row.get(2)?,
                target: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })
        .map_err(|err| format!("查询动作失败: {err}"))?;
    collect(rows, "读取动作")
}

fn collect<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    label: &str,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("{label}失败: {err}"))?);
    }
    Ok(out)
}

fn infer_effects(properties: &str, action: &Action) -> Vec<(&'static str, f64, &'static str)> {
    let mut out = Vec::new();
    match action.action.as_str() {
        "freezing_normal_weapon_attack" => {
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
                push_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
                push_number(&mut out, "freeze_count", normal, "冻结对手随机", "件", "件");
                push_number(&mut out, "freeze_duration", normal, "道具", "秒", "秒");
            }
        }
        "add_freeze_attack_stack" => {
            push_number(&mut out, "attack_bonus", properties, "所有武器+", "点", "点");
            if !out.iter().any(|(key, _, _)| *key == "attack_bonus") {
                push_number(&mut out, "attack_bonus", properties, "所有武器+", "攻击力", "点");
            }
            push_number(&mut out, "max_stacks", properties, "最多叠加", "次", "次");
        }
        "charged_healing_weapon_attack" => {
            if let Some(charged) = section_after(properties, "蓄力攻击") {
                push_number(&mut out, "charged_sword_cost", charged, "消耗", "点剑势", "点");
                push_number(&mut out, "charged_damage_multiplier", charged, "造成", "倍", "倍");
                push_number(&mut out, "charged_heal", charged, "生命恢复+", "点", "点");
            }
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
                push_number(&mut out, "normal_heal", normal, "生命恢复+", "点", "点");
            }
            push_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
        }
        "charged_sword_control_weapon_attack" => {
            if let Some(charged) = section_after(properties, "蓄力攻击") {
                push_number(&mut out, "charged_sword_cost", charged, "消耗", "点剑势", "点");
                push_number(&mut out, "charged_damage_multiplier", charged, "造成", "倍", "倍");
                push_number(&mut out, "charged_enemy_sword_loss", charged, "对手剑势-", "点", "点");
                push_number(&mut out, "freeze_count", charged, "冻结对手随机", "件", "件");
                push_number(&mut out, "freeze_duration", charged, "道具", "秒", "秒");
            }
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
            }
            push_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
        }
        "charge_self" => {
            push_number(&mut out, "charge_seconds", properties, "本武器充能", "秒", "秒");
        }
        "start_sword" => {
            push_number(&mut out, "start_sword", properties, "战斗初始剑势+", "点", "点");
        }
        "enemy_sword" => {
            push_number(&mut out, "enemy_sword_loss", properties, "对手剑势-", "点", "点");
        }
        _ => {}
    }
    out
}

fn push_number(
    out: &mut Vec<(&'static str, f64, &'static str)>,
    key: &'static str,
    text: &str,
    start: &str,
    end: &str,
    unit: &'static str,
) {
    if let Some(value) = number_between(text, start, end) {
        out.push((key, value, unit));
    }
}

fn insert_effect(
    conn: &Connection,
    short: &str,
    action: &Action,
    key: &str,
    value: f64,
    unit: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO equipment_action_effects
         (equipment_short, star, trigger, action, target, sort_order, effect_key, value, unit)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            short,
            action.star,
            action.trigger,
            action.action,
            action.target,
            action.sort_order,
            key,
            value,
            unit,
        ],
    )
    .map_err(|err| format!("写入动作效果参数失败: {err}"))?;
    Ok(())
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

fn parse_slash_group(token: &str) -> Option<Vec<f64>> {
    if !token.contains('/') {
        return None;
    }
    let values: Option<Vec<f64>> = token.split('/').map(|part| part.parse().ok()).collect();
    values.filter(|values| !values.is_empty())
}

fn section_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = after(text, marker)?;
    let stop = rest.find('；').unwrap_or(rest.len());
    Some(&rest[..stop])
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    text.find(marker).map(|idx| &text[idx + marker.len()..])
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
