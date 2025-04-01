mod ice;
use data_structs::{
    SerDeFile as _, ServerData,
    inventory::{DefaultClassesData, DefaultClassesDataReadable, ItemName},
    map::MapData,
    name_to_id,
    quest::QuestData,
    stats::{
        AllEnemyStats, AttackStats, AttackStatsReadable, ClassStatsStored, EnemyBaseStats,
        EnemyLevelBaseStats, NamedEnemyStats, PlayerStats, RaceModifierStored,
    },
};
use pso2packetlib::protocol::models::item_attrs;
use std::{
    env,
    error::Error,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use crate::ice::{IceFileInfo, IceWriter};

fn main() {
    let mut args = env::args();
    args.next();
    let filename = args.next().expect("Input filename");
    let filename = PathBuf::from(filename);

    let mut server_data = ServerData::default();

    // parse maps
    println!("Parsing maps...");
    let mut map_dir = filename.to_path_buf();
    map_dir.push("maps");
    find_data_dir(&map_dir, parse_map, &mut server_data).unwrap();

    // parse quests
    println!("Parsing quests...");
    let mut quest_dir = filename.to_path_buf();
    quest_dir.push("quests");
    find_data_dir(&quest_dir, parse_quest, &mut server_data).unwrap();

    // parse item names
    println!("Parsing item names...");
    let mut names_file = filename.to_path_buf();
    names_file.push("item_names");
    names_file = select_ext(names_file);
    if names_file.is_file() {
        let data = Vec::<ItemName>::load_file(&names_file).unwrap();
        server_data.item_params.names = data;
    }

    // parse item attributes
    println!("Parsing item attributes...");
    let mut attrs_file = filename.to_path_buf();
    attrs_file.push("item_attrs");
    attrs_file = select_ext(attrs_file);
    if attrs_file.is_file() {
        create_attr_files(&attrs_file, &mut server_data).unwrap();
    }

    // parse player stats
    println!("Parsing player stats...");
    let mut player_stats_dir = filename.to_path_buf();
    player_stats_dir.push("class_stats");
    server_data.player_stats = parse_player_stats(&player_stats_dir).unwrap();

    // parse enemy stats
    println!("Parsing enemy stats...");
    let mut base_enemy_stats_dir = filename.to_path_buf();
    let mut enemy_stats_dir = filename.to_path_buf();
    base_enemy_stats_dir.push("base_enemy_stats");
    base_enemy_stats_dir = select_ext(base_enemy_stats_dir);
    enemy_stats_dir.push("enemies");
    server_data.enemy_stats = parse_enemy_stats(&base_enemy_stats_dir, &enemy_stats_dir).unwrap();

    // parse attack stats
    println!("Parsing attack stats...");
    let mut attack_stats_dir = filename.to_path_buf();
    attack_stats_dir.push("attack_stats");
    server_data.attack_stats = parse_attack_stats(&attack_stats_dir).unwrap();

    // parse default class data
    println!("Parsing default classes data...");
    let mut class_data_dir = filename.to_path_buf();
    class_data_dir.push("class_data");
    server_data.default_classes = parse_default_classes(&class_data_dir).unwrap();

    println!("Saving data...");
    let mut out_filename = filename.to_path_buf();
    out_filename.push("com_data.mp");
    server_data.save_to_mp_comp(out_filename).unwrap();
}

fn parse_map(path: &Path, srv_data: &mut ServerData) -> Result<(), Box<dyn Error>> {
    let mut data_file = path.to_path_buf();
    data_file.push("data");
    data_file = select_ext(data_file);
    println!("\tParsing map data {}...", data_file.display());
    let mut data = MapData::load_file(&data_file)?;

    collect_map_data(path, &mut data)?;

    data_file.pop();
    let map_name = data_file.file_stem().unwrap().to_string_lossy().to_string();
    srv_data.maps.insert(map_name, data);
    Ok(())
}

fn collect_map_data(map_path: &Path, map: &mut MapData) -> Result<(), Box<dyn Error>> {
    // load lua files
    let mut lua_dir = map_path.to_path_buf();
    lua_dir.push("luas");
    if lua_dir.exists() {
        println!("\t\tParsing lua directory {}...", lua_dir.display());
        traverse_data_dir(lua_dir, &mut |p| {
            let lua = fs::read_to_string(p)?;
            println!("\t\t\tParsing lua {}...", p.display());
            let filename = p.file_stem().unwrap().to_string_lossy().to_string();
            map.luas.insert(filename, lua);
            Ok(())
        })?;
    }

    // load object files
    let mut object_dir = map_path.to_path_buf();
    object_dir.push("objects");
    if object_dir.exists() {
        println!("\t\tParsing object directory {}...", object_dir.display());
        traverse_data_dir(object_dir, &mut |p| {
            println!("\t\t\tParsing object {}...", p.display());
            let mut objects = Vec::load_file(p)?;
            map.objects.append(&mut objects);
            Ok(())
        })?;
    }

    // load transporters files
    let mut transporter_dir = map_path.to_path_buf();
    transporter_dir.push("transporters");
    if transporter_dir.exists() {
        println!(
            "\t\tParsing transporter directory {}...",
            transporter_dir.display()
        );
        traverse_data_dir(transporter_dir, &mut |p| {
            println!("\t\t\tParsing transporter {}...", p.display());
            let mut objects = Vec::load_file(p)?;
            map.transporters.append(&mut objects);
            Ok(())
        })?;
    }

    // load event files
    let mut event_dir = map_path.to_path_buf();
    event_dir.push("events");
    if event_dir.exists() {
        println!("\t\tParsing event directory {}...", event_dir.display());
        traverse_data_dir(event_dir, &mut |p| {
            println!("\t\t\tParsing event {}...", p.display());
            let mut objects = Vec::load_file(p)?;
            map.events.append(&mut objects);
            Ok(())
        })?;
    }

    // load npc files
    let mut npc_dir = map_path.to_path_buf();
    npc_dir.push("npcs");
    if npc_dir.exists() {
        println!("\t\tParsing NPC directory {}...", npc_dir.display());
        traverse_data_dir(npc_dir, &mut |p| {
            println!("\t\t\tParsing NPC {}...", p.display());
            let mut objects = Vec::load_file(p)?;
            map.npcs.append(&mut objects);
            Ok(())
        })?;
    }

    // populate zone settings
    let Some(init_zone) = map.zones.iter().find(|z| z.zone_id == map.init_map) else {
        return Err("No initial zone set".into());
    };
    map.map_data.settings = init_zone.settings.clone();
    let mut other_settings = vec![];
    for zone in map.zones.iter().filter(|z| !z.is_special_zone) {
        other_settings.push(zone.settings.clone());
    }
    map.map_data.other_settings = other_settings;

    Ok(())
}

fn parse_quest(path: &Path, srv_data: &mut ServerData) -> Result<(), Box<dyn Error>> {
    let mut data_file = path.to_path_buf();
    data_file.push("data");
    data_file = select_ext(data_file);
    println!("\tParsing quest data {}...", data_file.display());
    let mut data = QuestData::load_file(&data_file)?;

    // load map
    let mut map_dir = path.to_path_buf();
    map_dir.push("map");
    if map_dir.exists() {
        map_dir.push("map");
        map_dir = select_ext(map_dir);
        println!("\t\tParsing quest map data {}...", data_file.display());
        data.map = MapData::load_file(&map_dir)?;
        map_dir.pop();
        collect_map_data(&map_dir, &mut data.map)?;
    }
    // load enemy files
    let mut enemy_dir = path.to_path_buf();
    enemy_dir.push("enemies");
    if enemy_dir.exists() {
        println!("\t\tParsing enemy directory {}...", enemy_dir.display());
        traverse_data_dir(enemy_dir, &mut |p| {
            println!("\t\t\tParsing enemy {}...", p.display());
            let mut objects = Vec::load_file(p)?;
            data.enemies.append(&mut objects);
            Ok(())
        })?;
    }

    srv_data.quests.push(data);
    Ok(())
}

fn parse_player_stats(path: &Path) -> Result<PlayerStats, Box<dyn Error>> {
    let mut data = PlayerStats::default();

    // load level modifiers
    let mut level_mod_path = path.to_path_buf();
    level_mod_path.push("level_modifiers");
    level_mod_path = select_ext(level_mod_path);
    if level_mod_path.is_file() {
        println!(
            "\tParsing level modifier data {}...",
            level_mod_path.display()
        );
        let mod_data = RaceModifierStored::load_file(&level_mod_path)?;
        data.modifiers.push(mod_data.human_male);
        data.modifiers.push(mod_data.human_female);
        data.modifiers.push(mod_data.newman_male);
        data.modifiers.push(mod_data.newman_female);
        data.modifiers.push(mod_data.cast_male);
        data.modifiers.push(mod_data.cast_female);
        data.modifiers.push(mod_data.deuman_male);
        data.modifiers.push(mod_data.deuman_female);
    }

    // load class stats
    let mut max_class = 0;
    traverse_data_dir(path, &mut |p| {
        let file_name = p.file_name().unwrap().to_string_lossy();
        if file_name == "level_modifiers.json" || file_name == "level_modifiers.toml" {
            return Ok(());
        }
        println!("\tParsing class stats data {}...", p.display());
        let stats = ClassStatsStored::load_file(p)?;
        let class_int = stats.class as usize;
        if class_int >= max_class {
            max_class = class_int;
            data.stats.resize(class_int + 1, Default::default());
        }
        data.stats[class_int] = stats.stats;
        Ok(())
    })?;

    Ok(data)
}
fn duplicate_stats(mut stats: Vec<EnemyLevelBaseStats>) -> Vec<EnemyLevelBaseStats> {
    let mut last_stats = stats.remove(0);
    let mut new_stats = vec![last_stats.clone()];
    for stat in stats {
        for level in new_stats.last().unwrap().level + 1..stat.level {
            let mut new_stat = last_stats.clone();
            new_stat.level = level;
            new_stats.push(new_stat);
        }
        new_stats.push(stat.clone());
        last_stats = stat;
    }

    if new_stats.last().unwrap().level < 100 {
        for level in new_stats.last().unwrap().level + 1..100 {
            let mut new_stat = last_stats.clone();
            new_stat.level = level;
            new_stats.push(new_stat);
        }
    }

    new_stats
}

fn parse_enemy_stats(
    base_stats_path: &Path,
    stats_path: &Path,
) -> Result<AllEnemyStats, Box<dyn Error>> {
    let mut data = AllEnemyStats::default();

    // load base stats
    if base_stats_path.is_file() {
        println!(
            "\tParsing base enemy stats data {}...",
            base_stats_path.display()
        );

        let mut base = EnemyBaseStats::load_file(base_stats_path)?;
        let mut stats = std::mem::take(&mut base.levels);
        stats.sort_by(|a, b| a.level.cmp(&b.level));
        base.levels = duplicate_stats(stats);

        data.base = base;
    }

    // load class stats
    traverse_data_dir(stats_path, &mut |p| {
        println!("\tParsing enemy stats data {}...", p.display());
        let mut stats = NamedEnemyStats::load_file(p)?;

        {
            let base = &mut stats.stats;
            let mut stats = std::mem::take(&mut base.levels);
            stats.sort_by(|a, b| a.level.cmp(&b.level));
            base.levels = duplicate_stats(stats);
        }

        data.enemies.insert(stats.name, stats.stats);
        Ok(())
    })?;

    Ok(data)
}

fn parse_attack_stats(stats_path: &Path) -> Result<Vec<AttackStats>, Box<dyn Error>> {
    let mut data = vec![];

    // load stats
    traverse_data_dir(stats_path, &mut |p| {
        println!("\tParsing attack stats data {}...", p.display());
        let stats = Vec::<AttackStatsReadable>::load_file(p)?;
        for stat in stats {
            data.push(AttackStats {
                attack_id: name_to_id(&stat.attack_name),
                damage_id: name_to_id(&stat.damage_name),
                attack_type: stat.attack_type,
                defense_type: stat.defense_type,
                damage: stat.damage.into(),
            })
        }
        Ok(())
    })?;

    Ok(data)
}

fn parse_default_classes(classes_path: &Path) -> Result<DefaultClassesData, Box<dyn Error>> {
    let mut data = DefaultClassesData::default();

    // load stats
    traverse_data_dir(classes_path, &mut |p| {
        println!("\tParsing default class data {}...", p.display());
        let stats = DefaultClassesDataReadable::load_file(p)?;
        if stats.class as usize >= data.classes.len() {
            data.classes
                .resize(stats.class as usize + 1, Default::default());
        }
        data.classes[stats.class as usize] = stats.data;
        Ok(())
    })?;

    Ok(data)
}

fn find_data_dir<P, F>(
    path: P,
    callback: F,
    srv_data: &mut ServerData,
) -> Result<(), Box<dyn Error>>
where
    P: AsRef<Path>,
    F: Fn(&Path, &mut ServerData) -> Result<(), Box<dyn Error>> + Copy,
{
    // find data.json
    if fs::read_dir(&path)?.any(|p| p.unwrap().file_name().to_str().unwrap() == "data.json") {
        return callback(path.as_ref(), srv_data);
    }

    // find data.toml
    if fs::read_dir(&path)?.any(|p| p.unwrap().file_name().to_str().unwrap() == "data.toml") {
        return callback(path.as_ref(), srv_data);
    }

    let dir = fs::read_dir(path)?;
    for entry in dir {
        let entry = entry?.path();
        if entry.is_dir() {
            find_data_dir(entry, callback, srv_data)?;
        }
    }
    Ok(())
}

fn traverse_data_dir<P, F>(path: P, callback: &mut F) -> Result<(), Box<dyn Error>>
where
    P: AsRef<Path>,
    F: FnMut(&Path) -> Result<(), Box<dyn Error>>,
{
    if !path.as_ref().exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?.path();
        if entry.is_dir() {
            traverse_data_dir(entry, callback)?;
        } else if entry.is_file() {
            callback(&entry)?;
        }
    }
    Ok(())
}

fn create_attr_files(path: &Path, srv_data: &mut ServerData) -> Result<(), Box<dyn Error>> {
    let attrs = item_attrs::ItemAttributes::load_file(path)?;

    // PC attributes
    let outdata_pc = Cursor::new(vec![]);
    let attrs: item_attrs::ItemAttributesPC = attrs.into();
    srv_data.item_params.attrs = attrs.clone();
    let mut attrs_data_pc = Cursor::new(vec![]);
    attrs.write_attrs(&mut attrs_data_pc)?;
    attrs_data_pc.set_position(0);
    let mut ice_writer = IceWriter::new(outdata_pc)?;
    ice_writer.load_group(ice::Group::Group2);
    ice_writer.new_file(IceFileInfo {
        filename: "item_parameter.bin".into(),
        file_extension: "bin".into(),
        ..Default::default()
    })?;
    std::io::copy(&mut attrs_data_pc, &mut ice_writer)?;
    srv_data.item_params.pc_attrs = ice_writer.into_inner()?.into_inner();

    // Vita attributes
    let outdata_vita = Cursor::new(vec![]);
    let attrs: item_attrs::ItemAttributesVita = attrs.into();
    let mut attrs_data_vita = Cursor::new(vec![]);
    attrs.write_attrs(&mut attrs_data_vita)?;
    attrs_data_vita.set_position(0);
    let mut ice_writer = IceWriter::new(outdata_vita)?;
    ice_writer.load_group(ice::Group::Group2);
    ice_writer.new_file(IceFileInfo {
        filename: "item_parameter.bin".into(),
        file_extension: "bin".into(),
        ..Default::default()
    })?;
    std::io::copy(&mut attrs_data_vita, &mut ice_writer)?;
    srv_data.item_params.vita_attrs = ice_writer.into_inner()?.into_inner();

    Ok(())
}

fn select_ext<P: AsRef<Path>>(path: P) -> PathBuf {
    let mut path = path.as_ref().to_path_buf();
    path.set_extension("json");
    if path.exists() {
        return path;
    }
    path.set_extension("toml");
    path
}
