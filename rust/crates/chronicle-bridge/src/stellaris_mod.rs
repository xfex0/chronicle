//! HoI4 → Stellaris transition, last step: a Stellaris mod with Earth's empire.
//!
//! Strategy: never invent the parts of an empire we don't model. Chronicle copies the game's
//! own human empire definition from the player's Stellaris install
//! (`prescripted_countries/*.txt`, the block whose species class is "HUM"), replaces only the
//! fields the Bridge decides — name, adjective, authority, ethics, civics, origin,
//! spawn_enabled — and writes it as a local mod:
//!
//! ```text
//! Documents/Paradox Interactive/Stellaris/mod/chronicle_earth.mod      (launcher entry)
//! Documents/Paradox Interactive/Stellaris/mod/chronicle_earth/
//!     descriptor.mod
//!     prescripted_countries/chronicle_earth.txt
//!     localisation/<language>/chronicle_earth_l_<language>.yml          (UTF-8 with BOM)
//! ```
//!
//! Without an install, a built-in template is used (unverified, reported as a warning).
//! Authority / ethic / civic / origin keys: games/stellaris/vocabulary.yaml (verified on 3.14.15).

use std::path::{Path, PathBuf};

use paradox_parser::TopLevelIndex;
use serde::Serialize;

use crate::config::Vocabulary;
use crate::design::EmpireDesign;

pub const MOD_FOLDER: &str = "chronicle_earth";
pub const EMPIRE_KEY: &str = "chronicle_earth";
const LANGUAGES: [&str; 10] =
    ["english", "russian", "german", "french", "spanish", "polish", "braz_por", "japanese", "korean", "simp_chinese"];

/// Fields Chronicle sets; any of them in the template is removed first.
const REPLACED: [&str; 8] = ["name", "adjective", "spawn_enabled", "authority", "civics", "origin", "ethic", "government"];

const BUILTIN_TEMPLATE: &str = r#"chronicle_earth = {
	species = {
		class = "HUM"
		portrait = "human"
		name = "PRESCRIPTED_species_name_humans1"
		plural = "PRESCRIPTED_species_plural_humans1"
		adjective = "PRESCRIPTED_species_adjective_humans1"
		name_list = "HUMAN1"
		trait = "trait_adaptive"
		trait = "trait_nomadic"
		trait = "trait_wasteful"
	}
	room = "personality_federation_builders_room"
	planet_name = "NAME_Earth"
	planet_class = "pc_continental"
	system_name = "NAME_Sol"
	graphical_culture = "humanoid_01"
	city_graphical_culture = "humanoid_01"
}
"#;

#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the generated empire is not valid Clausewitz text: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ModOptions {
    pub empire_name: String,
    pub adjective: String,
    /// "3.14.15" → supported_version "v3.14.*"
    pub game_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModResult {
    pub mod_dir: PathBuf,
    pub launcher_file: PathBuf,
    pub files: Vec<PathBuf>,
    /// "game: <file>" when the player's install provided the template, else "builtin".
    pub template_source: String,
    pub warnings: Vec<String>,
}

/// The human empire block from the game's own files, if the install is available.
pub fn find_human_template(install_dir: &Path) -> Option<(PathBuf, String)> {
    let dir = install_dir.join("prescripted_countries");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("txt")))
        .collect();
    files.sort();
    for file in files {
        let Ok(bytes) = std::fs::read(&file) else { continue };
        let Ok(idx) = TopLevelIndex::build(&bytes) else { continue };
        for span in idx.spans.iter().filter(|s| s.is_block) {
            let body = &bytes[span.value_start..span.value_end];
            let text = String::from_utf8_lossy(body);
            if text.contains("\"HUM\"") || text.contains("class = HUM") || text.contains("class=\"HUM\"") {
                let block = String::from_utf8_lossy(&bytes[span.key_offset..span.value_end]).into_owned();
                return Some((file, block));
            }
        }
    }
    None
}

/// Net brace depth change of one line, ignoring braces inside quotes and after `#`.
fn brace_delta(line: &str) -> i32 {
    let mut d = 0;
    let mut in_str = false;
    for ch in line.chars() {
        match ch {
            '"' => in_str = !in_str,
            '#' if !in_str => break,
            '{' if !in_str => d += 1,
            '}' if !in_str => d -= 1,
            _ => {}
        }
    }
    d
}

fn line_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let end = t.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    let (key, rest) = t.split_at(end);
    (!key.is_empty() && rest.trim_start().starts_with('=')).then_some(key)
}

/// Replace the template's header key and the Bridge-controlled fields.
pub fn build_empire_block(template: &str, design: &EmpireDesign, vocab: &Vocabulary) -> Result<String, ModError> {
    let open = template.find('{').ok_or_else(|| ModError::Invalid("template has no block".into()))?;
    let close = template.rfind('}').ok_or_else(|| ModError::Invalid("template has no closing brace".into()))?;
    let body = &template[open + 1..close];

    let mut kept = String::new();
    let mut depth = 0i32;
    let mut skip = 0i32;
    for line in body.lines() {
        let delta = brace_delta(line);
        if skip > 0 {
            skip += delta;
            continue;
        }
        if depth == 0 && line_key(line).is_some_and(|k| REPLACED.contains(&k)) {
            if delta > 0 {
                skip = delta;
            }
            continue;
        }
        depth += delta;
        kept.push_str(line);
        kept.push('\n');
    }

    let authority = vocab.authorities.get(&design.authority).map(|a| a.key.clone()).unwrap_or(format!("auth_{}", design.authority));
    let origin = vocab.origins.get(&design.origin).map(|o| o.key.clone()).unwrap_or(format!("origin_{}", design.origin));
    let civics: Vec<String> = design.civics.iter().map(|c| format!("\"civic_{c}\"")).collect();
    let mut out = format!("{EMPIRE_KEY} = {{\n");
    out += &format!("\tname = \"{EMPIRE_KEY}_name\"\n");
    out += &format!("\tadjective = \"{EMPIRE_KEY}_adj\"\n");
    out += "\tspawn_enabled = no\n";
    out += &format!("\tauthority = \"{authority}\"\n");
    out += &format!("\tcivics = {{ {} }}\n", civics.join(" "));
    out += &format!("\torigin = \"{origin}\"\n");
    for e in &design.ethics {
        out += &format!("\tethic = \"ethic_{e}\"\n");
    }
    out += kept.trim_end_matches('\n');
    out += "\n}\n";

    // The result must parse, and must say what we meant.
    let parsed = paradox_parser::parse(out.as_bytes()).map_err(|e| ModError::Invalid(e.to_string()))?;
    let block = parsed.root.get(EMPIRE_KEY).and_then(|v| v.as_container()).ok_or_else(|| ModError::Invalid("no empire block".into()))?;
    let got = block.get("authority").and_then(|v| v.as_scalar()).map(|s| s.as_str().into_owned());
    if got.as_deref() != Some(authority.as_str()) || block.get_all("ethic").count() != design.ethics.len() {
        return Err(ModError::Invalid("edited block does not round-trip".into()));
    }
    Ok(out)
}

fn supported_version(v: Option<&str>) -> String {
    let parts: Vec<&str> = v.unwrap_or("").split('.').filter(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()).collect();
    match parts.as_slice() {
        [major, minor, ..] => format!("v{major}.{minor}.*"),
        [major] => format!("v{major}.*"),
        _ => "v3.*".to_string(),
    }
}

fn yml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Write the mod into `mod_root` (= Documents/Paradox Interactive/Stellaris/mod).
pub fn write_mod(
    mod_root: &Path,
    design: &EmpireDesign,
    vocab: &Vocabulary,
    install_dir: Option<&Path>,
    opts: &ModOptions,
) -> Result<ModResult, ModError> {
    let mut warnings = Vec::new();
    let (template, template_source) = match install_dir.and_then(find_human_template) {
        Some((file, block)) => (block, format!("game: {}", file.display())),
        None => {
            warnings.push(
                "Stellaris install not found: used Chronicle's built-in human template (unverified). \
                 Set the Stellaris game folder on the Games screen for an exact copy of the game's own definition."
                    .to_string(),
            );
            (BUILTIN_TEMPLATE.to_string(), "builtin".to_string())
        }
    };
    if !vocab.verified {
        let unverified: Vec<String> = design
            .civics
            .iter()
            .filter(|c| !vocab.civics.get(*c).is_some_and(|x| x.verified))
            .map(|c| format!("civic_{c}"))
            .chain(
                std::iter::once(&design.origin)
                    .filter(|o| !vocab.origins.get(*o).is_some_and(|x| x.verified))
                    .map(|o| vocab.origins.get(o).map(|x| x.key.clone()).unwrap_or_default()),
            )
            .collect();
        if !unverified.is_empty() {
            warnings.push(format!(
                "Not yet confirmed in a real save: {}. If the game rejects them, pick another civic/origin in the empire editor.",
                unverified.join(", ")
            ));
        }
    }
    let block = build_empire_block(&template, design, vocab)?;

    let dir = mod_root.join(MOD_FOLDER);
    std::fs::create_dir_all(dir.join("prescripted_countries"))?;
    let mut files = Vec::new();

    let empire_file = dir.join("prescripted_countries").join(format!("{EMPIRE_KEY}.txt"));
    std::fs::write(&empire_file, format!("# Generated by Chronicle — Modern Era Bridge (HoI4 → Stellaris)\n{block}"))?;
    files.push(empire_file);

    for lang in LANGUAGES {
        let ldir = dir.join("localisation").join(lang);
        std::fs::create_dir_all(&ldir)?;
        let file = ldir.join(format!("{EMPIRE_KEY}_l_{lang}.yml"));
        let text = format!(
            "\u{feff}l_{lang}:\n {EMPIRE_KEY}_name:0 \"{}\"\n {EMPIRE_KEY}_adj:0 \"{}\"\n",
            yml_escape(&opts.empire_name),
            yml_escape(&opts.adjective)
        );
        std::fs::write(&file, text)?;
        files.push(file);
    }

    let descriptor = format!(
        "name=\"Chronicle — {}\"\nversion=\"1.0\"\ntags={{\n\t\"Species\"\n}}\nsupported_version=\"{}\"\n",
        opts.empire_name.replace('"', "'"),
        supported_version(opts.game_version.as_deref())
    );
    let descriptor_file = dir.join("descriptor.mod");
    std::fs::write(&descriptor_file, &descriptor)?;
    files.push(descriptor_file);
    let launcher_file = mod_root.join(format!("{MOD_FOLDER}.mod"));
    let path_line = dir.display().to_string().replace('\\', "/");
    std::fs::write(&launcher_file, format!("{descriptor}path=\"{path_line}\"\n"))?;

    Ok(ModResult { mod_dir: dir, launcher_file, files, template_source, warnings })
}

/// Stellaris' mod folder next to its save folder (…/Stellaris/save games → …/Stellaris/mod).
pub fn mod_root_from_save_dir(save_dir: &Path) -> PathBuf {
    let base = if save_dir.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("save games")) {
        save_dir.parent().unwrap_or(save_dir)
    } else {
        save_dir
    };
    base.join("mod")
}

/// Default empire names by authority (editable in the UI).
pub fn suggested_name(design: &EmpireDesign) -> (String, String) {
    let name = match (design.authority.as_str(), design.origin.as_str()) {
        (_, "post_apocalyptic") => "Terran Remnant",
        ("democratic", _) => "Terran Federation",
        ("oligarchic", _) => "Terran Commonwealth",
        ("dictatorial", _) => "Terran Directorate",
        ("imperial", _) => "Terran Imperium",
        _ => "Terran Union",
    };
    (name.to_string(), "Terran".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{presets, run};

    /// Shape of a vanilla-style human block (one entry per line, nested blocks, a multi-line
    /// civics block and fields Chronicle must replace).
    const TEMPLATE: &str = r#"humans1 = {
	name = "EMPIRE_DESIGN_humans1"
	adjective = "PRESCRIPTED_adjective_humans1"
	spawn_enabled = always # comment with { brace
	ship_prefix = "PRESCRIPTED_ship_prefix_humans1"
	species = {
		class = "HUM"
		portrait = "human"
		trait = "trait_adaptive"
	}
	room = "personality_federation_builders_room"
	authority = "auth_democratic"
	civics = {
		"civic_beacon_of_liberty"
		"civic_idealistic_foundation"
	}
	origin = "origin_default"
	ethic = "ethic_egalitarian"
	ethic = "ethic_xenophile"
	ethic = "ethic_militarist"
	flags = { human_1 custom_start_screen }
	planet_name = "NAME_Earth"
	empire_flag = {
		icon = { category = "human" file = "flag_human_9.dds" }
		colors = { "dark_blue" "black" "null" "null" }
	}
}
"#;

    fn dictatorship() -> EmpireDesign {
        let start = presets().into_iter().find(|(n, _)| *n == "military_dictatorship").unwrap().1;
        run(&start, 45_819_283).unwrap().design
    }

    #[test]
    fn replaces_only_bridge_fields() {
        let d = dictatorship();
        let out = build_empire_block(TEMPLATE, &d, Vocabulary::builtin()).unwrap();
        let p = paradox_parser::parse(out.as_bytes()).unwrap();
        let b = p.root.get("chronicle_earth").unwrap().as_container().unwrap();
        assert_eq!(b.get("authority").unwrap().as_scalar().unwrap().as_str(), "auth_dictatorial");
        let ethics: Vec<String> = b.get_all("ethic").map(|v| v.as_scalar().unwrap().as_str().into_owned()).collect();
        assert_eq!(ethics, ["ethic_militarist", "ethic_authoritarian", "ethic_xenophobe"]);
        let civics: Vec<String> = b.get("civics").unwrap().as_container().unwrap().items()
            .map(|v| v.as_scalar().unwrap().as_str().into_owned()).collect();
        assert_eq!(civics, ["civic_distinguished_admiralty", "civic_nationalistic_zeal"]);
        assert_eq!(b.get("spawn_enabled").unwrap().as_scalar().unwrap().as_str(), "no");
        // everything else kept from the game's template
        assert!(b.get("species").is_some() && b.get("empire_flag").is_some() && b.get("flags").is_some());
        assert_eq!(b.get("ship_prefix").unwrap().as_scalar().unwrap().as_str(), "PRESCRIPTED_ship_prefix_humans1");
        assert_eq!(b.get_all("name").count(), 1, "old name removed, new name added");
        assert!(!out.contains("beacon_of_liberty"), "template civics removed");
    }

    #[test]
    fn writes_a_launcher_ready_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let install = tmp.path().join("Stellaris");
        std::fs::create_dir_all(install.join("prescripted_countries")).unwrap();
        std::fs::write(install.join("prescripted_countries/00_top.txt"), TEMPLATE).unwrap();
        let mod_root = mod_root_from_save_dir(&tmp.path().join("docs/Stellaris/save games"));
        std::fs::create_dir_all(&mod_root).unwrap();
        let d = dictatorship();
        let opts = ModOptions { empire_name: "Terran Directorate".into(), adjective: "Terran".into(), game_version: Some("3.14.15".into()) };
        let r = write_mod(&mod_root, &d, Vocabulary::builtin(), Some(&install), &opts).unwrap();
        assert!(r.template_source.starts_with("game: "));
        assert!(mod_root.ends_with("docs/Stellaris/mod"));
        let launcher = std::fs::read_to_string(&r.launcher_file).unwrap();
        assert!(launcher.contains("supported_version=\"v3.14.*\"") && launcher.contains("path=\""));
        let yml = std::fs::read(r.mod_dir.join("localisation/english/chronicle_earth_l_english.yml")).unwrap();
        assert!(yml.starts_with(&[0xEF, 0xBB, 0xBF]), "Stellaris localisation needs a UTF-8 BOM");
        assert!(String::from_utf8_lossy(&yml).contains("chronicle_earth_name:0 \"Terran Directorate\""));
        assert_eq!(r.files.len(), 1 + LANGUAGES.len() + 1);
    }

    #[test]
    fn builtin_template_without_install() {
        let tmp = tempfile::tempdir().unwrap();
        let start = presets()[0].1.clone();
        let d = run(&start, 1).unwrap().design;
        let opts = ModOptions { empire_name: "X \"Y\"".into(), adjective: "Z".into(), game_version: None };
        let r = write_mod(tmp.path(), &d, Vocabulary::builtin(), None, &opts).unwrap();
        assert_eq!(r.template_source, "builtin");
        assert!(!r.warnings.is_empty());
        assert!(std::fs::read_to_string(&r.launcher_file).unwrap().contains("supported_version=\"v3.*\""));
    }

    #[test]
    fn helpers() {
        assert_eq!(brace_delta("a = { b = \"}\" } # {"), 0);
        assert_eq!(line_key("\tethic = \"x\""), Some("ethic"));
        assert_eq!(line_key("\t\"civic_a\""), None);
        assert_eq!(supported_version(Some("3.14.15")), "v3.14.*");
    }
}
