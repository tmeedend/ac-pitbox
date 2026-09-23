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
use std::sync::RwLock;

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
    /// ISO 3166-1 alpha-2, from `ISO2`: what the front end translates the name
    /// from (TAXO§12). `None` for the four British nations, which AC flags on
    /// their own and ISO does not code — they are translated by key instead.
    pub iso2: Option<String>,
}

/// ISO 3166-1 alpha-3 → alpha-2, for every entry of the game table.
///
/// **Built by measurement, and the measurement is what makes it trustworthy.**
/// 188 of the 221 names match the runtime's English region name exactly; the
/// 29 that do not (`Czech Republic`, `Russian Federation`, `Turkey`,
/// `Taiwan, China`…) were written by hand. The name match alone was WRONG six
/// times, and silently: it picked retired codes that still carry the same
/// English name — `UK` for the United Kingdom, `FX` (Metropolitan France) for
/// France, `YU` for Serbia, `DY` for Benin, `HV` for Burkina Faso, `TP` for
/// Timor-Leste. Those are excluded, and `iso2_covers_the_whole_game_table`
/// guards the result.
///
/// A table rather than a crate: 217 pairs of a frozen standard, against a
/// dependency for one lookup.
const ISO2: &[(&str, &str)] = &[
    ("ABW", "AW"),
    ("AFG", "AF"),
    ("AGO", "AO"),
    ("AIA", "AI"),
    ("ALB", "AL"),
    ("AND", "AD"),
    ("ARE", "AE"),
    ("ARG", "AR"),
    ("ARM", "AM"),
    ("ASM", "AS"),
    ("ATA", "AQ"),
    ("ATG", "AG"),
    ("AUS", "AU"),
    ("AUT", "AT"),
    ("AZE", "AZ"),
    ("BDI", "BI"),
    ("BEL", "BE"),
    ("BEN", "BJ"),
    ("BFA", "BF"),
    ("BGD", "BD"),
    ("BGR", "BG"),
    ("BHR", "BH"),
    ("BHS", "BS"),
    ("BIH", "BA"),
    ("BLR", "BY"),
    ("BLZ", "BZ"),
    ("BMU", "BM"),
    ("BOL", "BO"),
    ("BRA", "BR"),
    ("BRB", "BB"),
    ("BRN", "BN"),
    ("BTN", "BT"),
    ("BWA", "BW"),
    ("CAF", "CF"),
    ("CAN", "CA"),
    ("CCK", "CC"),
    ("CHE", "CH"),
    ("CHL", "CL"),
    ("CHN", "CN"),
    ("CIV", "CI"),
    ("CMR", "CM"),
    ("COD", "CD"),
    ("COG", "CG"),
    ("COK", "CK"),
    ("COL", "CO"),
    ("COM", "KM"),
    ("CPV", "CV"),
    ("CRI", "CR"),
    ("CUB", "CU"),
    ("CYM", "KY"),
    ("CYP", "CY"),
    ("CZE", "CZ"),
    ("DEU", "DE"),
    ("DJI", "DJ"),
    ("DMA", "DM"),
    ("DNK", "DK"),
    ("DOM", "DO"),
    ("DZA", "DZ"),
    ("ECU", "EC"),
    ("EGY", "EG"),
    ("ERI", "ER"),
    ("ESH", "EH"),
    ("ESP", "ES"),
    ("EST", "EE"),
    ("ETH", "ET"),
    ("FIN", "FI"),
    ("FJI", "FJ"),
    ("FRA", "FR"),
    ("FRO", "FO"),
    ("FSM", "FM"),
    ("GAB", "GA"),
    ("GBR", "GB"),
    ("GEO", "GE"),
    ("GGY", "GG"),
    ("GHA", "GH"),
    ("GIB", "GI"),
    ("GIN", "GN"),
    ("GMB", "GM"),
    ("GNB", "GW"),
    ("GNQ", "GQ"),
    ("GRC", "GR"),
    ("GRD", "GD"),
    ("GRL", "GL"),
    ("GTM", "GT"),
    ("GUM", "GU"),
    ("GUY", "GY"),
    ("HKG", "HK"),
    ("HND", "HN"),
    ("HRV", "HR"),
    ("HTI", "HT"),
    ("HUN", "HU"),
    ("IDN", "ID"),
    ("IMN", "IM"),
    ("IND", "IN"),
    ("IRL", "IE"),
    ("IRN", "IR"),
    ("IRQ", "IQ"),
    ("ISL", "IS"),
    ("ISR", "IL"),
    ("ITA", "IT"),
    ("JAM", "JM"),
    ("JEY", "JE"),
    ("JOR", "JO"),
    ("JPN", "JP"),
    ("KAZ", "KZ"),
    ("KEN", "KE"),
    ("KGZ", "KG"),
    ("KHM", "KH"),
    ("KIR", "KI"),
    ("KNA", "KN"),
    ("KOR", "KR"),
    ("KWT", "KW"),
    ("LAO", "LA"),
    ("LBN", "LB"),
    ("LBR", "LR"),
    ("LCA", "LC"),
    ("LIE", "LI"),
    ("LKA", "LK"),
    ("LSO", "LS"),
    ("LTU", "LT"),
    ("LUX", "LU"),
    ("LVA", "LV"),
    ("MAC", "MO"),
    ("MAR", "MA"),
    ("MCO", "MC"),
    ("MDA", "MD"),
    ("MDG", "MG"),
    ("MDV", "MV"),
    ("MEX", "MX"),
    ("MHL", "MH"),
    ("MKD", "MK"),
    ("MLI", "ML"),
    ("MLT", "MT"),
    ("MMR", "MM"),
    ("MNE", "ME"),
    ("MNG", "MN"),
    ("MOZ", "MZ"),
    ("MRT", "MR"),
    ("MSR", "MS"),
    ("MTQ", "MQ"),
    ("MUS", "MU"),
    ("MWI", "MW"),
    ("MYS", "MY"),
    ("NAM", "NA"),
    ("NCL", "NC"),
    ("NER", "NE"),
    ("NGA", "NG"),
    ("NIC", "NI"),
    ("NLD", "NL"),
    ("NOR", "NO"),
    ("NPL", "NP"),
    ("NRU", "NR"),
    ("NZL", "NZ"),
    ("OMN", "OM"),
    ("PAK", "PK"),
    ("PAN", "PA"),
    ("PER", "PE"),
    ("PHL", "PH"),
    ("PLW", "PW"),
    ("PNG", "PG"),
    ("POL", "PL"),
    ("PRI", "PR"),
    ("PRT", "PT"),
    ("PRY", "PY"),
    ("PYF", "PF"),
    ("QAT", "QA"),
    ("ROU", "RO"),
    ("RUS", "RU"),
    ("RWA", "RW"),
    ("SAU", "SA"),
    ("SDN", "SD"),
    ("SEN", "SN"),
    ("SGP", "SG"),
    ("SLB", "SB"),
    ("SLE", "SL"),
    ("SLV", "SV"),
    ("SMR", "SM"),
    ("SOM", "SO"),
    ("SRB", "RS"),
    ("STP", "ST"),
    ("SUR", "SR"),
    ("SVK", "SK"),
    ("SVN", "SI"),
    ("SWE", "SE"),
    ("SWZ", "SZ"),
    ("SYC", "SC"),
    ("SYR", "SY"),
    ("TCA", "TC"),
    ("TCD", "TD"),
    ("TGO", "TG"),
    ("THA", "TH"),
    ("TJK", "TJ"),
    ("TKM", "TM"),
    ("TLS", "TL"),
    ("TON", "TO"),
    ("TTO", "TT"),
    ("TUN", "TN"),
    ("TUR", "TR"),
    ("TUV", "TV"),
    ("TWN", "TW"),
    ("TZA", "TZ"),
    ("UGA", "UG"),
    ("UKR", "UA"),
    ("URY", "UY"),
    ("USA", "US"),
    ("UZB", "UZ"),
    ("VCT", "VC"),
    ("VEN", "VE"),
    ("VGB", "VG"),
    ("VIR", "VI"),
    ("VNM", "VN"),
    ("VUT", "VU"),
    ("WSM", "WS"),
    ("YEM", "YE"),
    ("ZAF", "ZA"),
    ("ZMB", "ZM"),
    ("ZWE", "ZW"),
];

/// The alpha-2 code of an alpha-3 one, when ISO has one.
pub fn iso2_of(code3: &str) -> Option<&'static str> {
    ISO2.iter().find(|(c3, _)| *c3 == code3).map(|(_, c2)| *c2)
}

/// The game's table, kept for the country normalisation (TAXO§7.1), which runs
/// deep inside the harmonisation where no configuration is at hand.
///
/// Process-wide on purpose: it is read-only data of the game install, loaded
/// once at startup (`lib.rs`), and threading it through `harmonize::store`
/// would reach every importer for a lookup. **Empty is a valid state** — no
/// install configured, or in tests: the normalisation then keeps the aliases
/// only, as before it existed.
static KNOWN: RwLock<Vec<Nationality>> = RwLock::new(Vec::new());

pub fn set_known(list: Vec<Nationality>) {
    match KNOWN.write() {
        Ok(mut k) => *k = list,
        Err(e) => log::warn!("nationality table not installed: {e}"),
    }
}

/// Runs `f` over the known table (possibly empty).
pub fn with_known<T>(f: impl FnOnce(&[Nationality]) -> T) -> T {
    match KNOWN.read() {
        Ok(k) => f(&k),
        Err(_) => f(&[]),
    }
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
            let iso2 = iso2_of(&code).map(str::to_string);
            Nationality { code, name, flag, iso2 }
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
    /// The six codes the name match got wrong, and why the table exists.
    #[test]
    fn iso2_never_gives_a_retired_code() {
        for (code3, want) in [
            ("GBR", "GB"),
            ("FRA", "FR"),
            ("SRB", "RS"),
            ("BEN", "BJ"),
            ("BFA", "BF"),
            ("CZE", "CZ"),
        ] {
            assert_eq!(iso2_of(code3), Some(want), "{code3}");
        }
        let retired = [
            "AN", "BU", "CS", "DD", "DY", "FX", "HV", "NH", "RH", "SU", "TP", "UK", "VD", "YD", "YU", "ZR",
        ];
        assert!(
            ISO2.iter().all(|(_, c2)| !retired.contains(c2)),
            "no retired code in the table"
        );
        let mut seen = std::collections::HashSet::new();
        assert!(ISO2.iter().all(|(c3, _)| seen.insert(*c3)), "one line per alpha-3 code");
    }

    #[test]
    #[ignore = "dépend de l'installation Assetto Corsa de la machine"]
    fn iso2_covers_the_whole_game_table() {
        let root = Path::new(r"D:\SteamLibrary\steamapps\common\assettocorsa");
        let pairs = parse(&std::fs::read_to_string(root.join("launcher/themes/.base/ac.utils.js")).unwrap());
        let missing: Vec<_> = pairs
            .iter()
            .filter(|(c, _)| iso2_of(c).is_none())
            .map(|(c, _)| c.as_str())
            .collect();
        assert_eq!(
            missing,
            ["ENG", "NIR", "SCT", "WLS"],
            "only the British nations lack an ISO code"
        );
    }

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
