use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, Transaction, params};

const PENDING_FILE: &str = "待写入装备.md";
const DOC_MD: &str = "图片属性汇总表.md";
const DOC_CSV: &str = "图片属性汇总表.csv";

#[derive(Clone, Debug)]
struct Args {
    pending_path: PathBuf,
    db_path: PathBuf,
    md_path: PathBuf,
    csv_path: PathBuf,
}

#[derive(Clone, Debug)]
struct Equipment {
    short: String,
    full: String,
    kind: String,
    kind_label: String,
    width: i64,
    height: i64,
    properties: String,
    tags: Vec<String>,
    star_profiles: Vec<StarProfile>,
    actions: Vec<Action>,
}

#[derive(Clone, Debug)]
struct StarProfile {
    star: i64,
    attack: f64,
    interval: f64,
    normal_sword: i64,
    freeze_attack_bonus: Option<i64>,
    charged_enemy_sword: Option<i64>,
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

#[derive(Clone, Debug)]
struct DocRow {
    name: String,
    kind_label: String,
    bag_slots: String,
    properties: String,
    tags: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(env::args().skip(1))?;
    let pending = fs::read_to_string(&args.pending_path)
        .map_err(|err| format!("读取 {} 失败: {err}", args.pending_path.display()))?;
    let items = parse_pending(&pending)?;
    if items.is_empty() {
        println!("{} 没有待写入装备。", args.pending_path.display());
        return Ok(());
    }

    write_docs(&args, &items)?;
    write_database(&args, &items)?;
    fs::write(&args.pending_path, "")
        .map_err(|err| format!("清空 {} 失败: {err}", args.pending_path.display()))?;

    println!("已写入 {} 件装备：", items.len());
    for item in items {
        println!(
            "- {}（动作 {} 条，星级档位 {} 条）",
            item.full,
            item.actions.len(),
            item.star_profiles.len()
        );
    }
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut pending_path = PathBuf::from(PENDING_FILE);
    let mut db_path = PathBuf::from("equipment.sqlite");
    let mut md_path = PathBuf::from(DOC_MD);
    let mut csv_path = PathBuf::from(DOC_CSV);
    let mut iter = args.peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--pending" => pending_path = next_path(&mut iter, "--pending")?,
            "--db" => db_path = next_path(&mut iter, "--db")?,
            "--md" => md_path = next_path(&mut iter, "--md")?,
            "--csv" => csv_path = next_path(&mut iter, "--csv")?,
            value if value.starts_with("--pending=") => {
                pending_path = PathBuf::from(value.trim_start_matches("--pending="));
            }
            value if value.starts_with("--db=") => {
                db_path = PathBuf::from(value.trim_start_matches("--db="));
            }
            value if value.starts_with("--md=") => {
                md_path = PathBuf::from(value.trim_start_matches("--md="));
            }
            value if value.starts_with("--csv=") => {
                csv_path = PathBuf::from(value.trim_start_matches("--csv="));
            }
            value => return Err(format!("未知参数：{value}")),
        }
    }

    Ok(Args {
        pending_path,
        db_path,
        md_path,
        csv_path,
    })
}

fn next_path(
    iter: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<PathBuf, String> {
    iter.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} 需要路径"))
}

fn print_help() {
    println!("用法:");
    println!("  cargo run --bin write_pending_equipment");
    println!("  cargo run --bin write_pending_equipment -- --pending 待写入装备.md");
    println!();
    println!("说明:");
    println!("  从待写入装备.md 读取以 --- 分隔的装备，写入图片属性汇总表.md/.csv 和 equipment.sqlite。");
    println!("  属性中的 1/2/3/4 星斜杠数值会同步展开写入 equipment_star_profiles。");
    println!("  同名装备会覆盖，写入成功后会清空待写入文件。");
}

fn parse_pending(text: &str) -> Result<Vec<Equipment>, String> {
    let mut items = Vec::new();
    for block in text.split("---") {
        let lines: Vec<&str> = block
            .lines()
            .map(|line| line.trim().trim_start_matches('\u{feff}'))
            .filter(|line| !line.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        if lines.len() < 2 {
            return Err(format!("装备块缺少属性正文：{}", lines[0]));
        }

        let (full, kind_label, property_start) = parse_block_head(&lines)?;
        let properties = normalize_properties(&lines[property_start..]);
        let short = short_name(&full, &kind_label);
        let (kind, width, height) = kind_shape(&kind_label);
        let tags = infer_tags(&properties);
        let star_profiles = infer_star_profiles(&kind, &properties);
        let actions = infer_actions_for_equipment(&short, &full, &properties, &star_profiles);
        if actions.is_empty() {
            eprintln!("提示: {} 暂未匹配到可执行动作，只写入基础资料。", full);
        }

        items.push(Equipment {
            short,
            full,
            kind,
            kind_label,
            width,
            height,
            properties,
            tags,
            star_profiles,
            actions,
        });
    }
    Ok(items)
}

fn parse_block_head(lines: &[&str]) -> Result<(String, String, usize), String> {
    if lines.len() >= 2 && is_kind_label(lines[1]) {
        let mut property_start = 2;
        while property_start < lines.len() && is_meta_line(lines[property_start]) {
            property_start += 1;
        }
        return Ok((lines[0].to_string(), lines[1].to_string(), property_start));
    }

    let (full, kind_label) = parse_inline_header(lines[0])?;
    let mut property_start = 1;
    while property_start < lines.len() && is_meta_line(lines[property_start]) {
        property_start += 1;
    }
    Ok((full, kind_label, property_start))
}

fn parse_inline_header(header: &str) -> Result<(String, String), String> {
    let mut parts: Vec<&str> = header.split_whitespace().collect();
    let Some(kind_label) = parts.pop() else {
        return Err("装备标题为空".to_string());
    };
    if parts.is_empty() {
        return Err(format!("装备标题缺少名称：{header}"));
    }
    Ok((parts.join(""), kind_label.to_string()))
}

fn is_kind_label(line: &str) -> bool {
    matches!(line, "魂玉" | "护符" | "药品" | "护甲")
        || line.starts_with("近战武器")
}

fn is_meta_line(line: &str) -> bool {
    line.starts_with("战力:") || line.starts_with("战力：")
}

fn normalize_properties(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim_end_matches(';').trim_end_matches('；'))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("；")
}

fn short_name(full: &str, kind_label: &str) -> String {
    full.strip_suffix(kind_label)
        .filter(|short| !short.is_empty())
        .unwrap_or(full)
        .to_string()
}

fn kind_shape(kind_label: &str) -> (String, i64, i64) {
    if kind_label.starts_with("近战武器") {
        ("weapon".to_string(), 1, 3)
    } else if kind_label == "护符" {
        ("charm".to_string(), 2, 2)
    } else if kind_label == "药品" {
        ("potion".to_string(), 1, 1)
    } else if kind_label == "护甲" {
        ("armor".to_string(), 2, 3)
    } else {
        ("soul".to_string(), 1, 1)
    }
}

fn infer_tags(properties: &str) -> Vec<String> {
    let rules = [
        ("武器攻击力", "攻击力"),
        ("普通攻击", "普通攻击"),
        ("蓄力攻击", "蓄力攻击"),
        ("剑势", "剑势"),
        ("冻结", "冻结"),
        ("减速", "减速"),
        ("暴击", "暴击"),
        ("加速", "加速"),
        ("治疗", "治疗"),
        ("充能", "充能"),
        ("振刀", "振刀"),
        ("护甲", "护甲"),
        ("减伤", "减伤"),
        ("吸血", "吸血"),
        ("灼伤", "灼伤"),
        ("中毒", "中毒"),
    ];
    let mut tags = Vec::new();
    for (tag, keyword) in rules {
        if properties.contains(keyword) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn infer_actions_for_equipment(
    short: &str,
    full: &str,
    properties: &str,
    star_profiles: &[StarProfile],
) -> Vec<Action> {
    if star_profiles.is_empty() {
        return infer_actions(short, full, properties, 0);
    }

    (1..=4)
        .flat_map(|star| infer_actions(short, full, &properties_for_star(properties, star), star as i64))
        .collect()
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

    if properties.contains("攻击命中时") && properties.contains("对手剑势-") {
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

fn infer_star_profiles(kind: &str, properties: &str) -> Vec<StarProfile> {
    if !has_star_values(properties) {
        return Vec::new();
    }

    (1..=4)
        .map(|star| {
            let properties_for_star = properties_for_star(properties, star);
            StarProfile {
                star: star as i64,
                attack: base_attack_for_profile(kind, &properties_for_star),
                interval: number_between(&properties_for_star, "发动间隔", "，")
                    .or_else(|| number_between(&properties_for_star, "发动间隔", "；"))
                    .unwrap_or(0.0),
                normal_sword: normal_sword_for_profile(&properties_for_star),
                freeze_attack_bonus: freeze_attack_bonus_for_profile(&properties_for_star),
                charged_enemy_sword: charged_enemy_sword_for_profile(&properties_for_star),
                properties: format!("{star}星：{properties_for_star}"),
            }
        })
        .collect()
}

fn base_attack_for_profile(kind: &str, properties: &str) -> f64 {
    if kind != "weapon" || !properties.contains("星级属性") {
        return 0.0;
    }
    number_between(properties, "攻击力", "，")
        .or_else(|| number_between(properties, "攻击力", "；"))
        .unwrap_or(0.0)
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

fn normal_sword_for_profile(properties: &str) -> i64 {
    if let Some(normal) = after(properties, "普通攻击") {
        number_between(normal, "剑势+", "点").unwrap_or(0.0) as i64
    } else {
        0
    }
}

fn freeze_attack_bonus_for_profile(properties: &str) -> Option<i64> {
    if properties.contains("触发冻结") && properties.contains("所有武器+") {
        number_between(properties, "所有武器+", "点").map(|value| value as i64)
    } else {
        None
    }
}

fn charged_enemy_sword_for_profile(properties: &str) -> Option<i64> {
    if let Some(charged) = after(properties, "蓄力攻击") {
        number_between(charged, "对手剑势-", "点").map(|value| value as i64)
    } else {
        None
    }
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

fn write_docs(args: &Args, items: &[Equipment]) -> Result<(), String> {
    let rows = load_doc_rows(&args.csv_path)?;
    let by_name: BTreeMap<String, DocRow> = items
        .iter()
        .map(|item| {
            (
                item.full.clone(),
                DocRow {
                    name: item.full.clone(),
                    kind_label: item.kind_label.clone(),
                    bag_slots: format!("{}*{}", item.width, item.height),
                    properties: item.properties.clone(),
                    tags: item.tags.join(","),
                },
            )
        })
        .collect();

    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for row in rows {
        if let Some(replacement) = by_name.get(&row.name) {
            merged.push(replacement.clone());
            seen.insert(row.name);
        } else {
            merged.push(row);
        }
    }
    for item in by_name.values() {
        if !seen.contains(&item.name) {
            merged.push(item.clone());
        }
    }

    write_csv(&args.csv_path, &merged)?;
    write_md(&args.md_path, &merged)?;
    Ok(())
}

fn load_doc_rows(path: &PathBuf) -> Result<Vec<DocRow>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("读取 {} 失败: {err}", path.display()))?;
    let mut rows = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if idx == 0 || line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        if fields.len() != 5 {
            return Err(format!("{} 第 {} 行不是 5 列", path.display(), idx + 1));
        }
        rows.push(DocRow {
            name: fields[0].clone(),
            kind_label: fields[1].clone(),
            bag_slots: fields[2].clone(),
            properties: fields[3].clone(),
            tags: fields[4].clone(),
        });
    }
    Ok(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(ch),
        }
    }
    fields.push(field);
    fields
}

fn write_csv(path: &PathBuf, rows: &[DocRow]) -> Result<(), String> {
    let mut lines = vec!["name,type,bag_slots,properties,attribute_tags".to_string()];
    for row in rows {
        lines.push(
            [
                csv_field(&row.name),
                csv_field(&row.kind_label),
                csv_field(&row.bag_slots),
                csv_field(&row.properties),
                csv_field(&row.tags),
            ]
            .join(","),
        );
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .map_err(|err| format!("写入 {} 失败: {err}", path.display()))
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_md(path: &PathBuf, rows: &[DocRow]) -> Result<(), String> {
    let mut lines = vec![
        "| 名称 | 类型 | 背包占用 | 属性信息 | 关联附属属性 |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ];
    for row in rows {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            row.name, row.kind_label, row.bag_slots, row.properties, row.tags
        ));
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .map_err(|err| format!("写入 {} 失败: {err}", path.display()))
}

fn write_database(args: &Args, items: &[Equipment]) -> Result<(), String> {
    let mut conn = Connection::open(&args.db_path)
        .map_err(|err| format!("打开数据库 {} 失败: {err}", args.db_path.display()))?;
    ensure_equipment_actions_schema(&conn)?;
    create_action_effects_schema(&conn)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启数据库事务失败: {err}"))?;
    let sort_base: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM equipment",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("读取装备排序失败: {err}"))?;

    for (idx, item) in items.iter().enumerate() {
        let bag_slots = format!("{}*{}", item.width, item.height);
        let tags = item.tags.join(",");
        let default_attack = item
            .star_profiles
            .iter()
            .find(|profile| profile.star == 4)
            .map(|profile| profile.attack)
            .unwrap_or(0.0);
        let default_interval = item
            .star_profiles
            .iter()
            .find(|profile| profile.star == 4)
            .map(|profile| profile.interval)
            .unwrap_or(0.0);
        tx.execute(
            "INSERT INTO equipment
             (short, full, kind, kind_label, width, height, bag_slots, attack, interval, default_star, properties_text, attribute_tags_text, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 4, ?10, ?11, ?12)
             ON CONFLICT(short) DO UPDATE SET
                full = excluded.full,
                kind = excluded.kind,
                kind_label = excluded.kind_label,
                width = excluded.width,
                height = excluded.height,
                bag_slots = excluded.bag_slots,
                attack = excluded.attack,
                interval = excluded.interval,
                default_star = excluded.default_star,
                properties_text = excluded.properties_text,
                attribute_tags_text = excluded.attribute_tags_text",
            params![
                item.short,
                item.full,
                item.kind,
                item.kind_label,
                item.width,
                item.height,
                bag_slots,
                default_attack,
                default_interval,
                item.properties,
                tags,
                sort_base + idx as i64,
            ],
        )
        .map_err(|err| format!("写入 {} 失败: {err}", item.full))?;

        tx.execute(
            "DELETE FROM equipment_attribute_terms WHERE equipment_short = ?1",
            params![item.short],
        )
        .map_err(|err| format!("清理 {} 属性关联失败: {err}", item.full))?;
        for (tag_idx, tag) in item.tags.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO attribute_terms (term, description) VALUES (?1, '')",
                params![tag],
            )
            .map_err(|err| format!("写入术语 {tag} 失败: {err}"))?;
            tx.execute(
                "INSERT INTO equipment_attribute_terms (equipment_short, term, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![item.short, tag, tag_idx as i64],
            )
            .map_err(|err| format!("写入 {} 属性关联失败: {err}", item.full))?;
        }

        tx.execute(
            "DELETE FROM equipment_star_profiles WHERE short = ?1",
            params![item.short],
        )
        .map_err(|err| format!("清理 {} 星级档位失败: {err}", item.full))?;
        for profile in &item.star_profiles {
            tx.execute(
                "INSERT INTO equipment_star_profiles
                 (short, star, attack, interval, normal_sword, freeze_attack_bonus, charged_enemy_sword, properties_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    item.short,
                    profile.star,
                    profile.attack,
                    profile.interval,
                    profile.normal_sword,
                    profile.freeze_attack_bonus,
                    profile.charged_enemy_sword,
                    profile.properties,
                ],
            )
            .map_err(|err| format!("写入 {} 星级档位失败: {err}", item.full))?;
        }

        let actions_to_write = actions_for_database(&tx, item)?;

        tx.execute(
            "DELETE FROM equipment_actions WHERE equipment_short = ?1",
            params![item.short],
        )
        .map_err(|err| format!("清理 {} 动作失败: {err}", item.full))?;
        tx.execute(
            "DELETE FROM equipment_action_effects WHERE equipment_short = ?1",
            params![item.short],
        )
        .map_err(|err| format!("清理 {} 动作效果参数失败: {err}", item.full))?;
        for action in &actions_to_write {
            tx.execute(
                "INSERT INTO equipment_actions
                 (equipment_short, star, trigger, action, target, amount, duration, count, source_text, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    item.short,
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
            .map_err(|err| format!("写入 {} 动作失败: {err}", item.full))?;

            let props = if (1..=4).contains(&action.star) {
                properties_for_star(&item.properties, action.star as usize)
            } else {
                properties_for_star(&item.properties, 4)
            };
            for (key, value, unit) in infer_action_effects(&props, action) {
                insert_action_effect(&tx, &item.short, action, key, value, unit)?;
            }
        }
    }

    tx.commit()
        .map_err(|err| format!("提交数据库事务失败: {err}"))
}

fn actions_for_database(tx: &Transaction<'_>, item: &Equipment) -> Result<Vec<Action>, String> {
    let mut generic = load_generic_actions(tx, &item.short)?;
    let inferred_generic = if item.star_profiles.is_empty() {
        item.actions.clone()
    } else {
        infer_actions(
            &item.short,
            &item.full,
            &properties_for_star(&item.properties, 4),
            0,
        )
    };
    merge_actions(&mut generic, inferred_generic);

    if item.star_profiles.is_empty() {
        return Ok(generic);
    }

    let mut out = generic.clone();
    for star in 1..=4 {
        let mut star_actions = generic
            .iter()
            .cloned()
            .map(|mut action| {
                action.star = star;
                action
            })
            .collect::<Vec<_>>();
        let inferred_star = item
            .actions
            .iter()
            .filter(|action| action.star == star)
            .cloned()
            .collect::<Vec<_>>();
        merge_actions(&mut star_actions, inferred_star);
        out.extend(star_actions);
    }
    Ok(out)
}

fn load_generic_actions(tx: &Transaction<'_>, short: &str) -> Result<Vec<Action>, String> {
    let mut stmt = tx
        .prepare(
            "SELECT trigger, action, target, amount, duration, count, source_text, sort_order
             FROM equipment_actions
             WHERE equipment_short = ?1 AND star = 0
             ORDER BY sort_order, trigger",
        )
        .map_err(|err| format!("读取 {short} 通用动作失败: {err}"))?;
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
        .map_err(|err| format!("读取 {short} 通用动作失败: {err}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("解析 {short} 通用动作失败: {err}"))?);
    }
    Ok(out)
}

fn merge_actions(actions: &mut Vec<Action>, incoming: Vec<Action>) {
    for action in incoming {
        let key = action_key(&action);
        if let Some(existing) = actions.iter_mut().find(|existing| action_key(existing) == key) {
            *existing = action;
        } else {
            actions.push(action);
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

fn infer_action_effects(
    properties: &str,
    action: &Action,
) -> Vec<(&'static str, f64, &'static str)> {
    let mut out = Vec::new();
    match action.action.as_str() {
        "freezing_normal_weapon_attack" => {
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_effect_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
                push_effect_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
                push_effect_number(&mut out, "freeze_count", normal, "冻结对手随机", "件", "件");
                push_effect_number(&mut out, "freeze_duration", normal, "道具", "秒", "秒");
            }
        }
        "add_freeze_attack_stack" => {
            push_effect_number(&mut out, "attack_bonus", properties, "所有武器+", "点", "点");
            if !out.iter().any(|(key, _, _)| *key == "attack_bonus") {
                push_effect_number(&mut out, "attack_bonus", properties, "所有武器+", "攻击力", "点");
            }
            push_effect_number(&mut out, "max_stacks", properties, "最多叠加", "次", "次");
        }
        "charged_healing_weapon_attack" => {
            if let Some(charged) = section_after(properties, "蓄力攻击") {
                push_effect_number(&mut out, "charged_sword_cost", charged, "消耗", "点剑势", "点");
                push_effect_number(&mut out, "charged_damage_multiplier", charged, "造成", "倍", "倍");
                push_effect_number(&mut out, "charged_heal", charged, "生命恢复+", "点", "点");
            }
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_effect_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
                push_effect_number(&mut out, "normal_heal", normal, "生命恢复+", "点", "点");
            }
            push_effect_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
        }
        "charged_sword_control_weapon_attack" => {
            if let Some(charged) = section_after(properties, "蓄力攻击") {
                push_effect_number(&mut out, "charged_sword_cost", charged, "消耗", "点剑势", "点");
                push_effect_number(&mut out, "charged_damage_multiplier", charged, "造成", "倍", "倍");
                push_effect_number(&mut out, "charged_enemy_sword_loss", charged, "对手剑势-", "点", "点");
                push_effect_number(&mut out, "freeze_count", charged, "冻结对手随机", "件", "件");
                push_effect_number(&mut out, "freeze_duration", charged, "道具", "秒", "秒");
            }
            if let Some(normal) = section_after(properties, "普通攻击") {
                push_effect_number(&mut out, "normal_sword", normal, "剑势+", "点", "点");
            }
            push_effect_number(&mut out, "multi_trigger", properties, "多重触发", "；", "次");
        }
        "charge_self" => {
            push_effect_number(&mut out, "charge_seconds", properties, "本武器充能", "秒", "秒");
        }
        "start_sword" => {
            push_effect_number(&mut out, "start_sword", properties, "战斗初始剑势+", "点", "点");
        }
        "enemy_sword" => {
            push_effect_number(&mut out, "enemy_sword_loss", properties, "对手剑势-", "点", "点");
        }
        _ => {}
    }
    out
}

fn push_effect_number(
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

fn insert_action_effect(
    tx: &Transaction<'_>,
    short: &str,
    action: &Action,
    key: &str,
    value: f64,
    unit: &str,
) -> Result<(), String> {
    tx.execute(
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
    .map_err(|err| format!("写入 {short} 动作效果参数失败: {err}"))?;
    Ok(())
}

fn create_action_effects_schema(conn: &Connection) -> Result<(), String> {
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
