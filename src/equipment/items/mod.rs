use crate::equipment::{EquipmentSpec, StarProfile};

mod baizhan;
mod baizhuzhili;
mod bingpo;
mod bingsui;
mod chuanku;
mod cuipo;
mod dongjie;
mod fengrui;
mod fuminghuan;
mod hanbinghe;
mod hundun;
mod kuaiyi;
mod lingbohunyu;
mod longhu;
mod poshi;
mod shenjiangjia;
mod shenjingbao;
mod shixue;
mod tianxiang;
mod tongda;
mod tuna;
mod xuanzwu;
mod xuming;
mod zhanpo;
mod zhanshen;
mod zhudun;
mod zhushenling;

pub fn all() -> Vec<EquipmentSpec> {
    vec![
        baizhan::SPEC,
        zhanshen::SPEC,
        dongjie::SPEC,
        tianxiang::SPEC,
        bingsui::SPEC,
        shixue::SPEC,
        poshi::SPEC,
        fuminghuan::SPEC,
        cuipo::SPEC,
        fengrui::SPEC,
        xuming::SPEC,
        kuaiyi::SPEC,
        chuanku::SPEC,
        bingpo::SPEC,
        tuna::SPEC,
        xuanzwu::SPEC,
        hanbinghe::SPEC,
        shenjingbao::SPEC,
        zhudun::SPEC,
        tongda::SPEC,
        shenjiangjia::SPEC,
        longhu::SPEC,
        baizhuzhili::SPEC,
        zhanpo::SPEC,
        hundun::SPEC,
        zhushenling::SPEC,
        lingbohunyu::SPEC,
    ]
}

pub fn star_profile(short: &str, star: u8) -> Option<StarProfile> {
    let profiles = match short {
        "冰魄" => Some(bingpo::STAR_PROFILES.as_slice()),
        "龙弧" => Some(longhu::STAR_PROFILES.as_slice()),
        "烛神令" => Some(zhushenling::STAR_PROFILES.as_slice()),
        _ => None,
    }?;
    profiles
        .iter()
        .copied()
        .find(|profile| profile.star == star)
}
