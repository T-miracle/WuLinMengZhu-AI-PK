use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use rusqlite::Connection;

const REQUIRED_STARS: [i64; 4] = [1, 2, 3, 4];

#[derive(Debug)]
struct Args {
    db_path: PathBuf,
    show_complete: bool,
}

#[derive(Debug)]
struct EquipmentStars {
    short: String,
    full: String,
    kind_label: String,
    stars: BTreeSet<i64>,
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
    let items = load_equipment_stars(&conn)?;

    let mut missing = Vec::new();
    let mut complete = Vec::new();
    for item in items {
        if missing_stars(&item).is_empty() {
            complete.push(item);
        } else {
            missing.push(item);
        }
    }

    if missing.is_empty() {
        println!("所有装备都已有 1/2/3/4 星档位。");
    } else {
        println!("没有完整 1/2/3/4 星档位的装备：{} 件", missing.len());
        println!("| 装备 | 缩写 | 类型 | 已有星级 | 缺少星级 |");
        println!("|---|---|---|---|---|");
        for item in &missing {
            println!(
                "| {} | {} | {} | {} | {} |",
                item.full,
                item.short,
                empty_as_dash(&item.kind_label),
                stars_label(&item.stars),
                missing_label(item),
            );
        }
    }

    if args.show_complete {
        println!();
        println!("已完整写入 1/2/3/4 星档位的装备：{} 件", complete.len());
        for item in complete {
            println!("- {}（缩写：{}，类型：{}）", item.full, item.short, empty_as_dash(&item.kind_label));
        }
    }

    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut db_path = PathBuf::from("equipment.sqlite");
    let mut show_complete = false;
    let mut iter = args.peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--complete" | "-c" => show_complete = true,
            "--db" => {
                let Some(path) = iter.next() else {
                    return Err("--db 需要数据库路径".to_string());
                };
                db_path = PathBuf::from(path);
            }
            value if value.starts_with("--db=") => {
                db_path = PathBuf::from(value.trim_start_matches("--db="));
            }
            value => return Err(format!("未知参数：{value}")),
        }
    }

    Ok(Args {
        db_path,
        show_complete,
    })
}

fn print_help() {
    println!("用法:");
    println!("  cargo run --bin equipment_missing_stars");
    println!("  cargo run --bin equipment_missing_stars -- --complete");
    println!("  cargo run --bin equipment_missing_stars -- --db equipment.sqlite");
    println!();
    println!("说明:");
    println!("  列出 equipment 表中没有完整 1/2/3/4 星级档位的装备，并显示缺少哪些星级。");
}

fn load_equipment_stars(conn: &Connection) -> Result<Vec<EquipmentStars>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT e.short, e.full, COALESCE(e.kind_label, ''), s.star
             FROM equipment e
             LEFT JOIN equipment_star_profiles s ON s.short = e.short
             ORDER BY e.sort_order, e.short, s.star",
        )
        .map_err(|err| format!("准备查询失败: {err}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|err| format!("查询装备星级失败: {err}"))?;

    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| format!("读取装备星级失败: {err}"))?
    {
        let short: String = row.get(0).map_err(|err| format!("解析缩写失败: {err}"))?;
        let full: String = row.get(1).map_err(|err| format!("解析名称失败: {err}"))?;
        let kind_label: String = row.get(2).map_err(|err| format!("解析类型失败: {err}"))?;
        let star: Option<i64> = row.get(3).map_err(|err| format!("解析星级失败: {err}"))?;

        if let Some(last) = items.last_mut() {
            let last: &mut EquipmentStars = last;
            if last.short == short {
                if let Some(star) = star {
                    last.stars.insert(star);
                }
                continue;
            }
        }

        let mut stars = BTreeSet::new();
        if let Some(star) = star {
            stars.insert(star);
        }
        items.push(EquipmentStars {
            short,
            full,
            kind_label,
            stars,
        });
    }

    Ok(items)
}

fn missing_stars(item: &EquipmentStars) -> Vec<i64> {
    REQUIRED_STARS
        .into_iter()
        .filter(|star| !item.stars.contains(star))
        .collect()
}

fn stars_label(stars: &BTreeSet<i64>) -> String {
    if stars.is_empty() {
        return "-".to_string();
    }
    stars
        .iter()
        .map(|star| star.to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn missing_label(item: &EquipmentStars) -> String {
    missing_stars(item)
        .into_iter()
        .map(|star| star.to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}
