//! La liste des nationalités d'Assetto Corsa, et le drapeau de chacune.
//!
//! **Ce n'est pas un champ libre.** La nationalité d'une IA se choisit dans une
//! liste fermée, et le jeu en affiche le drapeau — ce que la cellule du plateau
//! traitait comme du texte quelconque.
//!
//! ## Ce que le relevé a donné
//!
//! - **La table est celle du jeu**, `launcher/themes/.base/ac.utils.js`, sous
//!   `$.Nationalities` : **221 entrées actives**, code ISO 3166-1 alpha-3 →
//!   nom anglais (`"ARG": "Argentina"`). Vingt-huit lignes y sont commentées —
//!   des territoires qu'AC a choisi de ne pas proposer — et elles restent
//!   exclues, puisque `//` n'est pas un espace.
//! - **Les drapeaux sont `content/gui/NationFlags/<CODE>.png`** : 222 fichiers,
//!   soit les 221 entrées **toutes pourvues** (vérifié : aucune sans fichier)
//!   plus `AC.png`, le repli du jeu.
//! - **Ce qui est stocké est le nom entier**, pas le code : `ui_skin.json` dit
//!   `"Argentina"`, et le tableau `Nationalities` d'un preset de grille CM aussi
//!   (« Brunei Darussalam » y a été relevé). La liste offerte est donc **par
//!   nom**, et le code ne sert qu'à trouver le drapeau.
//!
//! ## Le piège, et il est dans la table du jeu
//!
//! **« Congo » y apparaît deux fois** — `COD` et `COG`, les deux Congo, sous le
//! même libellé. Le nom ne peut donc pas désigner un drapeau sans ambiguïté.
//! C'est l'ambiguïté d'AC, pas la nôtre : on n'invente pas un libellé qu'il ne
//! connaît pas, la liste est dédoublonnée par nom et le premier code gagne pour
//! le drapeau.
//!
//! ## Pourquoi lire le fichier plutôt que recopier la table
//!
//! Les drapeaux viennent déjà de l'installation du jeu : y prendre aussi les
//! noms garde les deux alignés, y compris sur une install dont la table
//! différerait. Fichier absent ou illisible → liste vide, et la cellule
//! retombe sur la saisie libre : un enrichissement ne bloque rien.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Nationality {
    /// ISO 3166-1 alpha-3, tel que le jeu nomme ses fichiers de drapeau.
    pub code: String,
    /// Le nom anglais, **la valeur réellement stockée** par le jeu comme par
    /// les presets de Content Manager.
    pub name: String,
    /// Chemin absolu du PNG, à passer à `convertFileSrc`. `None` quand le
    /// fichier manque — l'entrée reste offerte, elle n'aura simplement pas
    /// d'image.
    pub flag: Option<String>,
}

/// Extrait les paires de `$.Nationalities`.
///
/// Le motif exige le guillemet **immédiatement** après l'indentation, ce qui
/// écarte gratuitement les lignes commentées : `//"ALA": "Åland Islands"` n'y
/// répond pas.
fn parse(js: &str) -> Vec<(String, String)> {
    let Some(start) = js.find("$.Nationalities") else {
        return Vec::new();
    };
    let block = &js[start..];
    let mut out = Vec::new();
    for line in block.lines().skip(1) {
        let trimmed = line.trim_start();
        // Fin de l'objet : on s'arrête là plutôt que de continuer à balayer
        // tout le fichier, où d'autres objets pourraient ressembler à ça.
        if trimmed.starts_with('}') {
            break;
        }
        if !trimmed.starts_with('"') {
            continue;
        }
        let mut parts = trimmed.splitn(2, ':');
        let (Some(code), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let code = code.trim().trim_matches('"');
        let name = name.trim().trim_end_matches(',').trim().trim_matches('"');
        if code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase()) && !name.is_empty() {
            out.push((code.to_string(), name.to_string()));
        }
    }
    out
}

/// Les nationalités offertes, dans l'ordre de la table du jeu (alphabétique par
/// nom), **dédoublonnées par nom** — voir le piège « Congo » en tête de module.
pub fn nationalities(ac_root: &Path) -> Vec<Nationality> {
    let js = match std::fs::read_to_string(ac_root.join("launcher/themes/.base/ac.utils.js")) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let flags = ac_root.join("content/gui/NationFlags");
    let mut seen = std::collections::HashSet::new();
    parse(&js)
        .into_iter()
        .filter(|(_, name)| seen.insert(name.clone()))
        .map(|(code, name)| {
            let png = flags.join(format!("{code}.png"));
            let flag = png.is_file().then(|| png.to_string_lossy().into_owned());
            Nationality { code, name, flag }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle protégée : les lignes commentées de la table ne sont pas offertes.
    /// Vingt-huit territoires y sont écartés par AC lui-même, et les reprendre
    /// proposerait des nationalités que le jeu ne connaît pas.
    #[test]
    fn commented_entries_are_not_offered() {
        let js = "\t$.Nationalities = {\n\t\t\"AFG\": \"Afghanistan\",\n\t\t//\"ALA\": \"Åland Islands\",\n\t\t\"ALB\": \"Albania\"\n\t};\n";
        let pairs = parse(js);
        assert_eq!(pairs.len(), 2, "la ligne commentée est écartée");
        assert_eq!(pairs[0], ("AFG".into(), "Afghanistan".into()));
        assert_eq!(pairs[1], ("ALB".into(), "Albania".into()));
    }

    /// Règle protégée : on s'arrête à la fin de l'objet. Le fichier contient
    /// des milliers de lignes après, et d'autres objets pourraient ressembler à
    /// une paire code/nom.
    #[test]
    fn parsing_stops_at_the_end_of_the_object() {
        let js = "$.Nationalities = {\n  \"FRA\": \"France\"\n};\nvar autre = {\n  \"XXX\": \"Pas un pays\"\n};\n";
        assert_eq!(parse(js).len(), 1);
    }

    /// Le **vrai** fichier de l'installation, et sa correspondance avec les
    /// drapeaux. Ignoré par défaut, comme tout ce qui dépend d'un état
    /// extérieur au dépôt :
    ///
    /// ```text
    /// cargo test --lib nationalities::tests::the_real_game_table -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "dépend de l'installation Assetto Corsa de la machine"]
    fn the_real_game_table() {
        let root = Path::new(r"D:\SteamLibrary\steamapps\common\assettocorsa");
        let list = nationalities(root);
        let without_flag: Vec<_> = list.iter().filter(|n| n.flag.is_none()).map(|n| &n.code).collect();
        println!("{} nationalités, {} sans drapeau", list.len(), without_flag.len());
        println!("sans drapeau : {without_flag:?}");
        assert!(list.len() > 200, "la table du jeu en compte 221");
        assert!(without_flag.is_empty(), "toutes doivent avoir leur drapeau");
    }
}
