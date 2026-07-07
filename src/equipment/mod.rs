use std::collections::{BTreeMap, HashMap};
use std::env;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{Def, Kind};

mod catalog;

#[derive(Clone, Copy, Debug)]
// 星级档位里会影响模拟的数值字段。
pub struct StarProfile {
    pub attack: f64,
    pub interval: f64,
    pub normal_sword: i32,
    pub freeze_attack_bonus: Option<i32>,
    pub charged_enemy_sword: Option<i32>,
}

#[derive(Clone, Debug)]
// 一条结构化后的装备行为，运行期直接解释执行。
pub struct ActionSpec {
    pub action: String,
    pub target: String,
    pub amount: Option<f64>,
    pub duration: Option<f64>,
    pub count: Option<i64>,
    pub source_text: String,
    pub effects: HashMap<String, f64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ActionKey {
    trigger: String,
    action: String,
    target: String,
    sort_order: i64,
}

const DB_FILE: &str = "equipment.sqlite";

// 读取基础装备定义，并在读取前确保本地 SQLite 已同步到最新种子数据。
pub fn specs() -> Result<Vec<Def>, String> {
    let mut conn = open_equipment_db()?;
    sync_catalog(&mut conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT short, kind, width, height, attack, interval
             FROM equipment
             ORDER BY sort_order",
        )
        .map_err(|err| format!("读取装备数据库失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            let kind_text: String = row.get(1)?;
            Ok(Def {
                short: leak_text(row.get::<_, String>(0)?),
                kind: kind_from_db(&kind_text),
                w: row.get::<_, i64>(2)? as usize,
                h: row.get::<_, i64>(3)? as usize,
                attack: row.get(4)?,
                interval: row.get(5)?,
            })
        })
        .map_err(|err| format!("读取装备数据库失败: {err}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("解析装备数据库失败: {err}"))?);
    }
    Ok(out)
}

// 查询某件装备某个星级的战斗数值配置。
pub fn star_profile(short: &str, star: u8) -> Option<StarProfile> {
    let mut conn = open_equipment_db().ok()?;
    sync_catalog(&mut conn).ok()?;

    conn.query_row(
        "SELECT attack, interval, normal_sword, freeze_attack_bonus, charged_enemy_sword
         FROM equipment_star_profiles
         WHERE short = ?1 AND star = ?2",
        params![short, star],
        |row| {
            Ok(StarProfile {
                attack: row.get(0)?,
                interval: row.get(1)?,
                normal_sword: row.get::<_, i64>(2)? as i32,
                freeze_attack_bonus: row.get::<_, Option<i64>>(3)?.map(|value| value as i32),
                charged_enemy_sword: row.get::<_, Option<i64>>(4)?.map(|value| value as i32),
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

// 一次性读出某件装备的全部触发动作，运行期不再频繁查库。
pub fn all_actions(short: &str, star: u8) -> Result<HashMap<String, Vec<ActionSpec>>, String> {
    let mut conn = open_equipment_db()?;
    sync_catalog(&mut conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT star, trigger, action, target, amount, duration, count, source_text, sort_order
             FROM equipment_actions
             WHERE equipment_short = ?1 AND star IN (0, ?2)
             ORDER BY star, trigger, sort_order",
        )
        .map_err(|err| format!("读取装备行为失败: {err}"))?;
    let rows = stmt
        .query_map(params![short, star as i64], |row| {
            let trigger = row.get::<_, String>(1)?;
            let action = row.get::<_, String>(2)?;
            let target = row.get::<_, String>(3)?;
            let sort_order = row.get::<_, i64>(8)?;
            Ok((
                ActionKey {
                    trigger,
                    action: action.clone(),
                    target: target.clone(),
                    sort_order,
                },
                row.get::<_, i64>(0)?,
                ActionSpec {
                    action,
                    target,
                    amount: row.get(4)?,
                    duration: row.get(5)?,
                    count: row.get(6)?,
                    source_text: row.get(7)?,
                    effects: HashMap::new(),
                },
            ))
        })
        .map_err(|err| format!("读取装备行为失败: {err}"))?;

    let mut merged: BTreeMap<ActionKey, (i64, ActionSpec)> = BTreeMap::new();
    for row in rows {
        let (key, action_star, action) = row.map_err(|err| format!("解析装备行为失败: {err}"))?;
        let replace = merged
            .get(&key)
            .is_none_or(|(existing_star, _)| action_star >= *existing_star);
        if replace {
            merged.insert(key, (action_star, action));
        }
    }

    let mut out: HashMap<String, Vec<ActionSpec>> = HashMap::new();
    for (key, (_, mut action)) in merged {
        action.effects = load_action_effects(&conn, short, star as i64, &key)?;
        out.entry(key.trigger).or_default().push(action);
    }
    Ok(out)
}

fn load_action_effects(
    conn: &Connection,
    short: &str,
    star: i64,
    key: &ActionKey,
) -> Result<HashMap<String, f64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT star, effect_key, value
             FROM equipment_action_effects
             WHERE equipment_short = ?1
               AND star IN (0, ?2)
               AND trigger = ?3
               AND action = ?4
               AND target = ?5
               AND sort_order = ?6
             ORDER BY star",
        )
        .map_err(|err| format!("读取动作效果参数失败: {err}"))?;
    let rows = stmt
        .query_map(
            params![
                short,
                star,
                key.trigger,
                key.action,
                key.target,
                key.sort_order
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?)),
        )
        .map_err(|err| format!("读取动作效果参数失败: {err}"))?;

    let mut out: HashMap<String, (i64, f64)> = HashMap::new();
    for row in rows {
        let (effect_star, effect_key, value) =
            row.map_err(|err| format!("解析动作效果参数失败: {err}"))?;
        let replace = out
            .get(&effect_key)
            .is_none_or(|(existing_star, _)| effect_star >= *existing_star);
        if replace {
            out.insert(effect_key, (effect_star, value));
        }
    }
    Ok(out
        .into_iter()
        .map(|(effect_key, (_, value))| (effect_key, value))
        .collect())
}

// 判断某件装备是否存在可选星级档位。
pub fn supports_star(short: &str) -> bool {
    let Ok(mut conn) = open_equipment_db() else {
        return false;
    };
    if sync_catalog(&mut conn).is_err() {
        return false;
    }

    conn.query_row(
        "SELECT COUNT(*) FROM equipment_star_profiles WHERE short = ?1",
        params![short],
        |row| row.get::<_, i64>(0),
    )
    .ok()
    .is_some_and(|count| count > 0)
}

// 数据库固定放在项目根目录，方便直接人工查看和维护。
fn open_equipment_db() -> Result<Connection, String> {
    let path = env::current_dir()
        .map_err(|err| format!("无法定位当前目录: {err}"))?
        .join(DB_FILE);
    Connection::open(path).map_err(|err| format!("打开装备数据库失败: {err}"))
}

// 每次启动都会补齐缺表、缺字段和缺 seed，但尽量不覆盖已有人工维护内容。
fn sync_catalog(conn: &mut Connection) -> Result<(), String> {
    create_base_schema(conn)?;
    ensure_equipment_columns(conn)?;
    ensure_column(
        conn,
        "equipment_star_profiles",
        "properties_text",
        "ALTER TABLE equipment_star_profiles ADD COLUMN properties_text TEXT",
    )?;
    ensure_equipment_actions_schema(conn)?;
    create_action_effects_schema(conn)?;

    let tx = conn
        .transaction()
        .map_err(|err| format!("开启装备数据库事务失败: {err}"))?;

    for term in catalog::ATTRIBUTE_TERMS {
        tx.execute(
            "INSERT OR IGNORE INTO attribute_terms (term, description) VALUES (?1, ?2)",
            params![term.term, term.description],
        )
        .map_err(|err| format!("写入术语说明失败: {err}"))?;

        tx.execute(
            "UPDATE attribute_terms
             SET description = ?1
             WHERE term = ?2",
            params![term.description, term.term],
        )
        .map_err(|err| format!("补全术语说明失败: {err}"))?;
    }
    for (idx, item) in catalog::EQUIPMENT.iter().enumerate() {
        let bag_slots = format!("{}*{}", item.w, item.h);
        let attribute_tags_text = item.attribute_tags.join(",");

        tx.execute(
            "INSERT OR IGNORE INTO equipment
             (short, full, kind, kind_label, width, height, bag_slots, attack, interval, default_star, properties_text, attribute_tags_text, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                item.short,
                item.full,
                kind_to_db(item.kind),
                item.kind_label,
                item.w as i64,
                item.h as i64,
                bag_slots,
                item.attack,
                item.interval,
                item.default_star as i64,
                item.properties,
                attribute_tags_text,
                idx as i64,
            ],
        )
        .map_err(|err| format!("写入装备数据库失败: {err}"))?;

        tx.execute(
            "UPDATE equipment SET
                full = COALESCE(NULLIF(full, ''), ?1),
                kind = COALESCE(NULLIF(kind, ''), ?2),
                kind_label = COALESCE(NULLIF(kind_label, ''), ?3),
                width = COALESCE(width, ?4),
                height = COALESCE(height, ?5),
                bag_slots = COALESCE(NULLIF(bag_slots, ''), ?6),
                attack = COALESCE(attack, ?7),
                interval = COALESCE(interval, ?8),
                default_star = COALESCE(default_star, ?9),
                properties_text = COALESCE(NULLIF(properties_text, ''), ?10),
                attribute_tags_text = COALESCE(NULLIF(attribute_tags_text, ''), ?11),
                sort_order = COALESCE(sort_order, ?12)
             WHERE short = ?13",
            params![
                item.full,
                kind_to_db(item.kind),
                item.kind_label,
                item.w as i64,
                item.h as i64,
                bag_slots,
                item.attack,
                item.interval,
                item.default_star as i64,
                item.properties,
                attribute_tags_text,
                idx as i64,
                item.short,
            ],
        )
        .map_err(|err| format!("补全装备数据库失败: {err}"))?;

        for (tag_idx, tag) in item.attribute_tags.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO equipment_attribute_terms (equipment_short, term, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![item.short, *tag, tag_idx as i64],
            )
            .map_err(|err| format!("写入装备术语关联失败: {err}"))?;
        }
    }

    for profile in catalog::STAR_PROFILES {
        tx.execute(
            "INSERT OR IGNORE INTO equipment_star_profiles
             (short, star, attack, interval, normal_sword, freeze_attack_bonus, charged_enemy_sword, properties_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                profile.short,
                profile.star as i64,
                profile.attack,
                profile.interval,
                profile.normal_sword as i64,
                profile.freeze_attack_bonus.map(|value| value as i64),
                profile.charged_enemy_sword.map(|value| value as i64),
                profile.properties,
            ],
        )
        .map_err(|err| format!("写入星级数据库失败: {err}"))?;

        tx.execute(
            "UPDATE equipment_star_profiles
             SET properties_text = COALESCE(NULLIF(properties_text, ''), ?1)
             WHERE short = ?2 AND star = ?3",
            params![profile.properties, profile.short, profile.star as i64],
        )
        .map_err(|err| format!("补全星级属性说明失败: {err}"))?;
    }

    tx.execute(
        "DELETE FROM equipment_actions
         WHERE action IN ('icepo_attack', 'longhu_attack', 'zhushenling_attack')",
        [],
    )
    .map_err(|err| format!("清理旧武器行为失败: {err}"))?;
    tx.execute(
        "DELETE FROM equipment_actions
         WHERE trigger = 'battle_start'
           AND action = 'sword'
           AND equipment_short IN ('天响', '振魄', '混沌', '烛神令')",
        [],
    )
    .map_err(|err| format!("清理旧开局剑势行为失败: {err}"))?;
    tx.execute(
        "DELETE FROM equipment_actions
         WHERE equipment_short = '神将甲'
           AND trigger = 'battle_start'
           AND action = 'armor'
           AND target = 'self'
           AND source_text = '神将甲护甲'",
        [],
    )
    .map_err(|err| format!("清理旧装备行为失败: {err}"))?;
    for action in catalog::EQUIPMENT_ACTIONS {
        tx.execute(
            "INSERT OR IGNORE INTO equipment_actions
             (equipment_short, star, trigger, action, target, amount, duration, count, source_text, sort_order)
             VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                action.equipment_short,
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
        .map_err(|err| format!("写入装备行为失败: {err}"))?;
    }

    tx.commit()
        .map_err(|err| format!("提交装备数据库事务失败: {err}"))?;
    Ok(())
}

// 所有表和视图的最小初始化结构。
// 初始化最小表结构；后续字段演进通过补列完成。
fn create_base_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS equipment (
            short TEXT PRIMARY KEY,
            full TEXT NOT NULL,
            kind TEXT NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            attack REAL NOT NULL,
            interval REAL NOT NULL,
            default_star INTEGER NOT NULL,
            sort_order INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS equipment_star_profiles (
            short TEXT NOT NULL,
            star INTEGER NOT NULL,
            attack REAL NOT NULL,
            interval REAL NOT NULL,
            normal_sword INTEGER NOT NULL,
            freeze_attack_bonus INTEGER,
            charged_enemy_sword INTEGER,
            properties_text TEXT,
            PRIMARY KEY (short, star),
            FOREIGN KEY (short) REFERENCES equipment(short) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS attribute_terms (
            term TEXT PRIMARY KEY,
            description TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS equipment_attribute_terms (
            equipment_short TEXT NOT NULL,
            term TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (equipment_short, term),
            FOREIGN KEY (equipment_short) REFERENCES equipment(short) ON DELETE CASCADE,
            FOREIGN KEY (term) REFERENCES attribute_terms(term)
        );
        CREATE TABLE IF NOT EXISTS equipment_actions (
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
        CREATE TABLE IF NOT EXISTS equipment_action_effects (
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
        );
        CREATE VIEW IF NOT EXISTS equipment_star_manual_view AS
        SELECT
            e.short,
            e.full,
            s.star,
            s.attack,
            s.interval,
            s.normal_sword,
            s.freeze_attack_bonus,
            s.charged_enemy_sword,
            s.properties_text,
            e.sort_order
        FROM equipment_star_profiles s
        JOIN equipment e ON e.short = s.short
        ORDER BY e.sort_order, s.star;
        CREATE VIEW IF NOT EXISTS equipment_manual_view AS
        SELECT
            short,
            full,
            kind_label,
            bag_slots,
            attack,
            interval,
            default_star,
            properties_text,
            attribute_tags_text,
            sort_order
        FROM equipment
        ORDER BY sort_order;",
    )
    .map_err(|err| format!("初始化装备数据库失败: {err}"))
}

// 兼容旧库：逐列补齐后续演进新增的字段。
// 旧库升级时逐列补齐，避免破坏已有人工编辑内容。
fn ensure_equipment_columns(conn: &Connection) -> Result<(), String> {
    ensure_column(
        conn,
        "equipment",
        "kind_label",
        "ALTER TABLE equipment ADD COLUMN kind_label TEXT",
    )?;
    ensure_column(
        conn,
        "equipment",
        "bag_slots",
        "ALTER TABLE equipment ADD COLUMN bag_slots TEXT",
    )?;
    ensure_column(
        conn,
        "equipment",
        "properties_text",
        "ALTER TABLE equipment ADD COLUMN properties_text TEXT",
    )?;
    ensure_column(
        conn,
        "equipment",
        "attribute_tags_text",
        "ALTER TABLE equipment ADD COLUMN attribute_tags_text TEXT",
    )?;
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

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<(), String> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&pragma)
        .map_err(|err| format!("读取表结构失败: {err}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|err| format!("读取表结构失败: {err}"))?;
    while let Some(row) = rows
        .next()
        .map_err(|err| format!("读取表结构失败: {err}"))?
    {
        let name: String = row.get(1).map_err(|err| format!("读取表结构失败: {err}"))?;
        if name == column {
            return Ok(());
        }
    }
    conn.execute(ddl, [])
        .map_err(|err| format!("更新表结构失败: {err}"))?;
    Ok(())
}

// 程序里的装备类型和数据库里的文本类型互相转换。
fn kind_to_db(kind: Kind) -> &'static str {
    match kind {
        Kind::Weapon => "weapon",
        Kind::Charm => "charm",
        Kind::Potion => "potion",
        Kind::Soul => "soul",
        Kind::Armor => "armor",
    }
}

fn kind_from_db(kind: &str) -> Kind {
    match kind {
        "weapon" => Kind::Weapon,
        "charm" => Kind::Charm,
        "potion" => Kind::Potion,
        "soul" => Kind::Soul,
        "armor" => Kind::Armor,
        _ => Kind::Soul,
    }
}

// SQLite 读出来的是运行期字符串；这里提升到静态生命周期，便于沿用现有事件系统。
fn leak_text(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_terms_include_crit_burn_and_poison() {
        let mut conn = open_equipment_db().expect("open equipment db");
        sync_catalog(&mut conn).expect("sync equipment catalog");

        let rows = [
            ("暴击", "有一定几率造成2倍伤害。"),
            (
                "灼伤",
                "每秒受到等同于当前灼伤层数的伤害，每秒层数减少10%。",
            ),
            ("中毒", "每秒受到等同于当前中毒层数的伤害，无视护甲。"),
        ];

        for (term, expected) in rows {
            let actual: String = conn
                .query_row(
                    "SELECT description FROM attribute_terms WHERE term = ?1",
                    params![term],
                    |row| row.get(0),
                )
                .expect("term exists");
            assert_eq!(actual, expected);
        }
    }
}
