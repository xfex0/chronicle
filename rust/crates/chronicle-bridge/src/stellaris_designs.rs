//! HoI4 → Stellaris, the reliable way: add Earth's empire to the player's OWN empire designs.
//!
//! `Documents/Paradox Interactive/Stellaris/user_empire_designs_v3.4.txt` holds every empire the
//! player saved in the in-game designer; the selection screen always lists them — no mod, no
//! playset, no version warning. Format taken from a real 4.5.2 "Cygnus" file:
//!
//! ```text
//! "Name"=
//! {
//!     key="Name"
//!     ship_prefix={ key="…" }
//!     species={ class portrait species_name={ key literal=yes } species_plural species_adjective
//!               name_list gender trait… }
//!     name={ key="Name" literal=yes }   adjective={ … }
//!     authority government is_nomadic planet_name={…} planet_class system_name={…} initializer
//!     graphical_culture city_graphical_culture empire_flag={ icon background colors{6} }
//!     ruler={ gender name={ full_names={ key } } portrait texture evolution_mask attachment clothes
//!             trait leader_class }
//!     spawn_as_fallen ignore_portrait_duplication room spawn_enabled ethic… civics={…} origin
//! }
//! ```
//!
//! Human details (species, flag, ruler, Sol) are the game's own UNE values from the 4.5.2 files.
//! The file is backed up before every change; an entry with the same name is replaced, other
//! designs are left untouched byte for byte.

use std::path::{Path, PathBuf};

use paradox_parser::TopLevelIndex;
use serde::Serialize;

use crate::config::Vocabulary;
use crate::design::EmpireDesign;

pub const DESIGNS_FILE: &str = "user_empire_designs_v3.4.txt";

#[derive(Debug, thiserror::Error)]
pub enum DesignsError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the empire designs file could not be read: {0}")]
    Parse(String),
    #[error("the generated design is not valid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignsResult {
    pub file: PathBuf,
    /// Copy of the file as it was before (None when the file did not exist yet).
    pub backup: Option<PathBuf>,
    pub empire_name: String,
    /// An earlier design with the same name was replaced.
    pub replaced: bool,
    pub warnings: Vec<String>,
}

fn government_for(authority: &str) -> &'static str {
    match authority {
        "democratic" => "gov_representative_democracy",
        "oligarchic" => "gov_executive_committee",
        "dictatorial" => "gov_military_dictatorship",
        "imperial" => "gov_star_empire",
        _ => "gov_representative_democracy",
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The design block in the 4.x user-designs format.
pub fn design_block(design: &EmpireDesign, vocab: &Vocabulary, empire_name: &str, adjective: &str) -> String {
    let authority = vocab.authorities.get(&design.authority).map(|a| a.key.clone()).unwrap_or(format!("auth_{}", design.authority));
    let origin = vocab.origins.get(&design.origin).map(|o| o.key.clone()).unwrap_or(format!("origin_{}", design.origin));
    let name = quote(empire_name);
    let adj = quote(adjective);
    let mut s = String::new();
    s += &format!("{name}=\n{{\n");
    s += &format!("\tkey={name}\n");
    s += "\tship_prefix=\n\t{\n\t\tkey=\"PRESCRIPTED_ship_prefix_humans1\"\n\t}\n";
    s += "\tspecies=\n\t{\n";
    s += "\t\tclass=\"HUM\"\n\t\tportrait=\"human\"\n";
    s += "\t\tspecies_name=\n\t\t{\n\t\t\tkey=\"PRESCRIPTED_species_name_humans1\"\n\t\t}\n";
    s += "\t\tspecies_plural=\n\t\t{\n\t\t\tkey=\"PRESCRIPTED_species_plural_humans1\"\n\t\t}\n";
    s += "\t\tspecies_adjective=\n\t\t{\n\t\t\tkey=\"PRESCRIPTED_species_adjective_humans1\"\n\t\t}\n";
    s += "\t\tname_list=\"HUMAN1\"\n\t\tgender=not_set\n";
    for t in ["trait_adaptive", "trait_nomadic", "trait_wasteful", "trait_organic"] {
        s += &format!("\t\ttrait=\"{t}\"\n");
    }
    s += "\t}\n";
    s += &format!("\tname=\n\t{{\n\t\tkey={name}\n\t\tliteral=yes\n\t}}\n");
    s += &format!("\tadjective=\n\t{{\n\t\tkey={adj}\n\t\tliteral=yes\n\t}}\n");
    s += &format!("\tauthority=\"{authority}\"\n");
    s += &format!("\tgovernment=\"{}\"\n", government_for(&design.authority));
    s += "\tis_nomadic=no\n";
    s += "\tplanet_name=\n\t{\n\t\tkey=\"NAME_Earth\"\n\t}\n";
    s += "\tplanet_class=\"pc_continental\"\n";
    s += "\tsystem_name=\n\t{\n\t\tkey=\"NAME_Sol\"\n\t}\n";
    s += "\tinitializer=\"sol_system_initializer\"\n";
    s += "\tgraphical_culture=\"humanoid_01\"\n\tcity_graphical_culture=\"humanoid_01\"\n";
    s += "\tempire_flag=\n\t{\n\t\ticon=\n\t\t{\n\t\t\tcategory=\"human\"\n\t\t\tfile=\"flag_human_9.dds\"\n\t\t}\n";
    s += "\t\tbackground=\n\t\t{\n\t\t\tcategory=\"backgrounds\"\n\t\t\tfile=\"00_solid.dds\"\n\t\t}\n";
    s += "\t\tcolors=\n\t\t{\n\t\t\t\"blue\"\n\t\t\t\"black\"\n\t\t\t\"null\"\n\t\t\t\"null\"\n\t\t\t\"null\"\n\t\t\t\"null\"\n\t\t}\n\t}\n";
    s += "\truler=\n\t{\n\t\tgender=female\n";
    s += "\t\tname=\n\t\t{\n\t\t\tfull_names=\n\t\t\t{\n\t\t\t\tkey=\"PRESCRIPTED_ruler_name_humans1\"\n\t\t\t}\n\t\t}\n";
    s += "\t\tportrait=\"human_female_05\"\n\t\ttexture=0\n\t\tevolution_mask=0\n\t\tattachment=1\n\t\tclothes=0\n";
    s += "\t\ttrait=\"trait_ruler_eye_for_talent\"\n\t\tleader_class=\"official\"\n\t}\n";
    s += "\tspawn_as_fallen=no\n\tignore_portrait_duplication=yes\n";
    s += "\troom=\"personality_federation_builders_room\"\n";
    s += "\tspawn_enabled=no\n";
    for e in &design.ethics {
        s += &format!("\tethic=\"ethic_{e}\"\n");
    }
    s += "\tcivics=\n\t{\n";
    for c in &design.civics {
        s += &format!("\t\t\"civic_{c}\"\n");
    }
    s += "\t}\n";
    s += &format!("\torigin=\"{origin}\"\n}}\n");
    s
}

/// Add (or replace) the design in `stellaris_docs/user_empire_designs_v3.4.txt`.
/// `stellaris_docs` = Documents/Paradox Interactive/Stellaris (the parent of "save games").
pub fn write_user_design(
    stellaris_docs: &Path,
    design: &EmpireDesign,
    vocab: &Vocabulary,
    empire_name: &str,
    adjective: &str,
) -> Result<DesignsResult, DesignsError> {
    let block = design_block(design, vocab, empire_name, adjective);
    // the block must parse and contain what we meant
    let parsed = paradox_parser::parse(block.as_bytes()).map_err(|e| DesignsError::Invalid(e.to_string()))?;
    let entry = parsed.root.get(empire_name).and_then(|v| v.as_container()).ok_or_else(|| DesignsError::Invalid("no design".into()))?;
    if entry.get_all("ethic").count() != design.ethics.len() || entry.get("government").is_none() {
        return Err(DesignsError::Invalid("design does not round-trip".into()));
    }

    let file = stellaris_docs.join(DESIGNS_FILE);
    std::fs::create_dir_all(stellaris_docs)?;
    let existing = if file.exists() { std::fs::read(&file)? } else { Vec::new() };
    let mut backup = None;
    if !existing.is_empty() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let b = stellaris_docs.join(format!("{DESIGNS_FILE}.chronicle-{stamp}.bak"));
        std::fs::write(&b, &existing)?;
        backup = Some(b);
    }

    let bom: &[u8] = if existing.starts_with(&[0xEF, 0xBB, 0xBF]) { &[0xEF, 0xBB, 0xBF] } else { &[] };
    let body = &existing[bom.len()..];
    let idx = TopLevelIndex::build(body).map_err(|e| DesignsError::Parse(e.to_string()))?;
    let mut out: Vec<u8> = bom.to_vec();
    let mut replaced = false;
    match idx.spans.iter().find(|s| s.is_block && s.key == empire_name) {
        Some(span) => {
            out.extend_from_slice(&body[..span.key_offset]);
            out.extend_from_slice(block.trim_end().as_bytes());
            out.extend_from_slice(&body[span.value_end..]);
            replaced = true;
        }
        None => {
            out.extend_from_slice(body);
            if !body.is_empty() && !body.ends_with(b"\n") {
                out.push(b'\n');
            }
            out.extend_from_slice(block.as_bytes());
        }
    }
    // Never leave a file the game cannot read.
    paradox_parser::parse(&out[bom.len()..]).map_err(|e| DesignsError::Invalid(e.to_string()))?;
    std::fs::write(&file, &out)?;

    let mut warnings = Vec::new();
    if stellaris_docs.join("mod").join("chronicle_earth.mod").exists() {
        warnings.push("old_chronicle_mod_installed".to_string());
    }
    Ok(DesignsResult { file, backup, empire_name: empire_name.to_string(), replaced, warnings })
}

/// Documents/Paradox Interactive/Stellaris from its "save games" folder.
pub fn stellaris_docs_from_save_dir(save_dir: &Path) -> PathBuf {
    if save_dir.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("save games")) {
        save_dir.parent().unwrap_or(save_dir).to_path_buf()
    } else {
        save_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{presets, run};

    /// Shape of the player's real 4.5.2 file (shortened).
    const EXISTING: &str = "\"Юґґот\"=\n{\n\tkey=\"Юґґот\"\n\tname=\n\t{\n\t\tkey=\"Юґґот\"\n\t\tliteral=yes\n\t}\n\tauthority=\"auth_imperial\"\n\tgovernment=\"gov_star_empire\"\n\tethic=\"ethic_militarist\"\n\tethic=\"ethic_fanatic_spiritualist\"\n\tcivics=\n\t{\n\t\t\"civic_mining_guilds\"\n\t\t\"civic_nationalistic_zeal\"\n\t}\n\torigin=\"origin_necrophage\"\n}\n";

    fn democracy() -> EmpireDesign {
        let start = presets().into_iter().find(|(n, _)| *n == "global_democracy").unwrap().1;
        run(&start, 45_819_283).unwrap().design
    }

    #[test]
    fn appends_without_touching_other_designs_and_replaces_on_rerun() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(DESIGNS_FILE), EXISTING).unwrap();
        let d = democracy();
        let r = write_user_design(tmp.path(), &d, Vocabulary::builtin(), "Terran Federation", "Terran").unwrap();
        assert!(!r.replaced && r.backup.as_ref().unwrap().exists());
        let text = std::fs::read_to_string(&r.file).unwrap();
        assert!(text.starts_with(EXISTING), "the player's own designs are kept byte for byte");
        let p = paradox_parser::parse(text.as_bytes()).unwrap();
        let ours = p.root.get("Terran Federation").unwrap().as_container().unwrap();
        assert_eq!(ours.get("government").unwrap().as_scalar().unwrap().as_str(), "gov_representative_democracy");
        assert_eq!(ours.get("origin").unwrap().as_scalar().unwrap().as_str(), "origin_default");
        let civics: Vec<String> = ours.get("civics").unwrap().as_container().unwrap().items()
            .map(|v| v.as_scalar().unwrap().as_str().into_owned()).collect();
        assert_eq!(civics, ["civic_beacon_of_liberty", "civic_technocracy"]);
        assert!(p.root.get("Юґґот").is_some());

        // again: replaced, not duplicated
        let again = write_user_design(tmp.path(), &d, Vocabulary::builtin(), "Terran Federation", "Terran").unwrap();
        assert!(again.replaced);
        let text = std::fs::read_to_string(&again.file).unwrap();
        assert_eq!(text.matches("\"Terran Federation\"=").count(), 1);
        assert!(text.starts_with(EXISTING));
    }

    #[test]
    fn creates_the_file_when_missing_and_keeps_a_bom() {
        let tmp = tempfile::tempdir().unwrap();
        let r = write_user_design(tmp.path(), &democracy(), Vocabulary::builtin(), "Terra", "Terran").unwrap();
        assert!(r.backup.is_none());
        assert!(std::fs::read_to_string(&r.file).unwrap().starts_with("\"Terra\"="));

        let tmp = tempfile::tempdir().unwrap();
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(EXISTING.as_bytes());
        std::fs::write(tmp.path().join(DESIGNS_FILE), &with_bom).unwrap();
        let r = write_user_design(tmp.path(), &democracy(), Vocabulary::builtin(), "Terra", "Terran").unwrap();
        assert!(std::fs::read(&r.file).unwrap().starts_with(&[0xEF, 0xBB, 0xBF]));
    }

    #[test]
    fn docs_folder_from_save_folder() {
        assert_eq!(stellaris_docs_from_save_dir(Path::new("/d/Stellaris/save games")), PathBuf::from("/d/Stellaris"));
    }
}
