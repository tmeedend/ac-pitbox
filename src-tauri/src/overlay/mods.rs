//! Mods, their versions and their history (§3): the rows the library is built on.

use super::*;

// --- Structures exposées au frontend ---------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModRow {
    pub id_interne: String,
    pub kind: String,
    pub brand: Option<String>,
    pub display_name: Option<String>,
    pub year: Option<i64>,
    pub car_class: Option<String>,
    pub category: Option<String>,
    /// Catégories de circuit (§5), multi-valué, ordonnées par priorité.
    /// Vide pour une voiture (qui utilise `category`).
    pub categories: Vec<String>,
    pub country: Option<String>,
    pub is_favorite: bool,
    pub active_version_id: Option<String>,
    pub version_count: i64,
    /// `None` pour le contenu de base (§4, `is_stock`) : la date en base est
    /// l'instant du réindex, sans rapport avec une vraie date d'ajout — pas de
    /// meilleure source disponible (mtime du filesystem = date d'installation
    /// du jeu, tout aussi dénué de sens). Mieux vaut l'absence explicite
    /// qu'une date fausse affichée comme si elle était fiable.
    pub created_at: Option<String>,
    /// Tags lus dans le fichier (origine « fichier mod », lecture seule).
    pub tags_from_mod: Vec<String>,
    /// Tags déduits par l'ontologie (origine « règle »).
    pub tags_from_rule: Vec<String>,
    /// Tags ajoutés à la main (origine « manuel »).
    pub tags_manual: Vec<String>,
    pub drivetrain: Option<String>,
    pub engine_pos: Option<String>,
    pub aspiration: Option<String>,
    pub engine_config: Option<String>,
    pub gearbox: Option<String>,
    /// Pack d'origine commun aux mods d'une même archive multi-voitures (§4.4).
    pub source_pack: Option<String>,
    /// URL d'origine (rempli plus tard par l'extension, §4.4).
    pub source_url: Option<String>,
    /// Auteur de la version active (colonne §6.2).
    pub author: Option<String>,
    /// Label de version de la version active (colonne §6.2).
    pub active_version_label: Option<String>,
    /// Date de dernière mise à jour = import de la version la plus récente (§6.2).
    pub updated_at: Option<String>,
    /// Layouts de la version active (colonne circuits §6.2).
    pub layouts: Vec<String>,
    /// Extensions CSP de la version active (colonne circuits §6.2).
    pub csp_features: Vec<String>,
    /// Nom saisi par l'utilisateur (§5bis.3), `None` si aucun. `display_name`
    /// ci-dessus vaut déjà celui-ci quand il existe : ce champ ne sert qu'à
    /// SAVOIR qu'il y a une surcharge (proposer d'y renoncer, pré-remplir le
    /// champ d'édition), jamais à l'affichage courant.
    pub display_name_user: Option<String>,
    /// Nom annoncé par le `ui_*.json` du mod, que `display_name` masque dès
    /// qu'une surcharge existe. Sert à montrer à quoi on reviendrait.
    pub display_name_file: Option<String>,
    /// Description saisie par l'utilisateur (§5bis.3). Contrairement au nom,
    /// la description native n'est pas en base — elle est relue dans le
    /// `ui_*.json` à chaque affichage — donc l'arbitrage se fait côté
    /// `library.rs`, pas en SQL.
    pub description_user: Option<String>,
    /// Indexé depuis `content/` : vit dans le dossier du jeu, sans version en
    /// bibliothèque — lecture seule, non désactivable (§8.1). Vrai pour le
    /// contenu de base Kunos **comme** pour un mod installé hors Pit Box :
    /// c'est ce qui fait que tout ce qui protège l'un protège l'autre (lecture
    /// des vignettes dans `content/`, refus d'activation, absence de date
    /// d'ajout…). La distinction se lit sur `is_unmanaged`.
    pub is_stock: bool,
    /// Mod installé hors Pit Box (§8.2) : présent dans `content/` comme
    /// un vrai dossier, mais absent de la table du contenu officiel
    /// ([`crate::kunos_dates::is_official`]). Toujours accompagné de
    /// `is_stock`. Contrairement au contenu de base, ce n'est **pas** du
    /// contenu de jeu : il ne reçoit ni couche, ni import par-dessus, et il
    /// n'est jamais sauvegardé/effacé de `content/` — l'app le laisse
    /// strictement où l'utilisateur l'a mis, définitivement. Le faire passer
    /// sous gestion suppose que l'utilisateur retire lui-même le dossier du
    /// jeu et importe le mod.
    pub is_unmanaged: bool,
    /// Note libre (REFONTE§9). Distincte de `description_user` : celle-ci
    /// surcharge ce que dit le fichier du mod, donc la vider veut dire
    /// « reviens au fichier » ; une note n'a pas de valeur d'origine, donc
    /// vide veut dire vide.
    pub notes_user: Option<String>,
    /// Date de publication estimée de la version active (§6.2).
    pub published_at: Option<String>,
    /// Taille sur disque cumulée de toutes les versions, octets (§10).
    /// `None` tant qu'aucune n'a été calculée (mod importé avant cette
    /// fonctionnalité, à rattraper via « Réindexer » + recalcul de taille).
    pub size_bytes: Option<i64>,
    /// Where the five spec fields above come from, when it is worth a sign
    /// (R5 of FICHE§3): `"rules"` for a value deduced from the tags, `"user"`
    /// for a correction. Absent means read in the mod's own files.
    pub tech_marks: std::collections::BTreeMap<String, String>,
    /// In the showcase (ESPACE§4.1): its active version is a skeleton.
    /// Resolved in `MOD_SELECT` like the other active-version columns, so a
    /// list of cards does not ask the base twice per card.
    pub showcase: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRow {
    pub id: String,
    pub mod_id: String,
    pub version_label: Option<String>,
    pub author: Option<String>,
    pub imported_at: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub content_signature: Option<String>,
    pub csp_features: Vec<String>,
    pub skins: Vec<String>,
    pub layouts: Vec<String>,
    pub tags_from_mod: Vec<String>,
    /// Date de publication estimée depuis les dates de fichiers (§6.2).
    pub published_at: Option<String>,
    /// Taille sur disque de cette version, octets (§10). For a version in the
    /// showcase, its size before its files went (ESPACE§4.1).
    pub size_bytes: Option<i64>,
    /// Archive/dossier source conservé en bibliothèque (§10/§11), si le
    /// réglage était activé à l'import. `None` = non conservé.
    pub kept_archive_path: Option<String>,
    /// `"full"` or `"skeleton"` (ESPACE§4.1): a skeleton kept only what the
    /// lists need, and never goes into the game.
    pub content_state: String,
    /// When the version went into the showcase, and what that freed.
    pub freed_at: Option<String>,
    pub freed_bytes: Option<i64>,
    /// Where the archive came from (ESPACE§8.2): the site, never a signed link.
    pub source_site: Option<String>,
    pub source_file_name: Option<String>,
}

/// [`VersionRow::content_state`] of a version that kept its files.
pub const CONTENT_FULL: &str = "full";
/// [`VersionRow::content_state`] of a version in the showcase (ESPACE§4.1).
pub const CONTENT_SKELETON: &str = "skeleton";

impl VersionRow {
    pub fn is_skeleton(&self) -> bool {
        self.content_state == CONTENT_SKELETON
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub timestamp: String,
    pub event: String,
    pub details: String,
}

pub(super) fn json_arr(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

// --- Écritures --------------------------------------------------------------

/// Insère le mod s'il n'existe pas (ne touche pas aux champs overlay-éditables
/// existants en cas de ré-import).
#[allow(clippy::too_many_arguments)]
pub fn upsert_mod(
    conn: &Connection,
    id_interne: &str,
    kind: &str,
    brand: Option<&str>,
    display_name: Option<&str>,
    identity_hash: &str,
    year: Option<i64>,
    created_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO mods (id_interne, kind, brand, display_name, identity_hash, year, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(id_interne) DO UPDATE SET
               brand = COALESCE(excluded.brand, mods.brand),
               display_name = COALESCE(excluded.display_name, mods.display_name),
               identity_hash = excluded.identity_hash,
               year = COALESCE(mods.year, excluded.year)"#,
        params![id_interne, kind, brand, display_name, identity_hash, year, created_at],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert_version(
    conn: &Connection,
    id: &str,
    mod_id: &str,
    version_label: Option<&str>,
    author: Option<&str>,
    imported_at: &str,
    library_path: &str,
    source_archive: Option<&str>,
    content_signature: &str,
    csp_features: &[String],
    skins: &[String],
    layouts: &[String],
    tags_from_mod: &[String],
    published_at: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO versions
           (id, mod_id, version_label, author, imported_at, library_path,
            source_archive, content_signature, csp_features, skins, layouts, tags_from_mod,
            published_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)"#,
        params![
            id,
            mod_id,
            version_label,
            author,
            imported_at,
            library_path,
            source_archive,
            content_signature,
            serde_json::to_string(csp_features).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(skins).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(layouts).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(tags_from_mod).unwrap_or_else(|_| "[]".into()),
            published_at,
        ],
    )?;
    Ok(())
}

/// Renseigne la taille sur disque d'une version, octets (§10). Séparé de
/// `insert_version` : calculée à l'import juste après la copie/le déplacement
/// en bibliothèque (le dossier final n'existe qu'à cet instant), et sur
/// demande explicite en réindexation (potentiellement coûteux à grande échelle,
/// d'où une case à cocher dédiée plutôt qu'un recalcul systématique).
pub fn update_version_size(conn: &Connection, version_id: &str, size_bytes: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET size_bytes = ?2 WHERE id = ?1",
        params![version_id, size_bytes],
    )?;
    Ok(())
}

/// Rafraîchit les champs d'un mod dérivés du `ui_*.json` (réindexation) sans
/// toucher aux champs overlay-éditables. N'écrase que si une nouvelle valeur
/// est fournie (préserve la valeur existante si le fichier ne la contient pas).
pub fn update_mod_reindexed_fields(
    conn: &Connection,
    id: &str,
    brand: Option<&str>,
    display_name: Option<&str>,
    year: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET
             brand = COALESCE(?2, brand),
             display_name = COALESCE(?3, display_name),
             year = COALESCE(?4, year)
         WHERE id_interne = ?1",
        params![id, brand, display_name, year],
    )?;
    Ok(())
}

/// `meta` key of [`strip_invisible_from_stored_texts`]: set once it has run.
const META_INVISIBLE_STRIPPED: &str = "invisible_chars_stripped";

/// What the base stored of the `ui_*.json` before their reading dropped the
/// invisible characters (`uijson::strip_invisible`), cleaned the same way -
/// the 14 "No Hesi Traffic" cars kept a U+1D17A in front of their name
/// otherwise, since a name is only read again on a reindex. Once per base (a
/// `meta` marker), and the number of values changed is returned.
pub fn strip_invisible_from_stored_texts(conn: &Connection) -> rusqlite::Result<usize> {
    if get_meta(conn, META_INVISIBLE_STRIPPED)?.is_some() {
        return Ok(0);
    }
    // The columns a `ui_*.json` fills, JSON lists included: the character is
    // never part of their structure, only of the texts inside.
    let tables: [(&str, &str, &[&str]); 2] = [
        ("mods", "id_interne", &["brand", "display_name", "car_class", "country"]),
        (
            "versions",
            "id",
            &["version_label", "author", "layouts", "tags_from_mod"],
        ),
    ];
    let mut changed = 0;
    for (table, key, columns) in tables {
        for column in columns {
            let mut stmt = conn.prepare(&format!(
                "SELECT {key}, {column} FROM {table} WHERE {column} IS NOT NULL"
            ))?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (id, value) in rows {
                if let std::borrow::Cow::Owned(clean) = crate::uijson::strip_invisible(&value) {
                    conn.execute(
                        &format!("UPDATE {table} SET {column} = ?2 WHERE {key} = ?1"),
                        params![id, clean],
                    )?;
                    changed += 1;
                }
            }
        }
    }
    set_meta(conn, META_INVISIBLE_STRIPPED, "1")?;
    Ok(changed)
}

/// Rafraîchit les champs d'une version dérivés du `ui_*.json`/inspection
/// (réindexation), même logique que `update_mod_reindexed_fields`.
#[allow(clippy::too_many_arguments)]
pub fn update_version_reindexed_fields(
    conn: &Connection,
    version_id: &str,
    version_label: Option<&str>,
    author: Option<&str>,
    csp_features: &[String],
    skins: &[String],
    layouts: &[String],
    tags_from_mod: &[String],
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET
             version_label = COALESCE(?2, version_label),
             author = COALESCE(?3, author),
             csp_features = ?4,
             skins = ?5,
             layouts = ?6,
             tags_from_mod = ?7
         WHERE id = ?1",
        params![
            version_id,
            version_label,
            author,
            serde_json::to_string(csp_features).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(skins).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(layouts).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(tags_from_mod).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

pub fn set_active_version(conn: &Connection, mod_id: &str, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET active_version_id = ?2 WHERE id_interne = ?1",
        params![mod_id, version_id],
    )?;
    Ok(())
}

/// Écrit le résultat d'harmonisation (§5) dans l'overlay. brand/country ne sont
/// écrasés que si une valeur est fournie ; tags_from_rule/car_class/category
/// reflètent toujours les règles.
///
/// The five spec fields (drivetrain, aspiration…) are no longer written here:
/// for a car they are the cache of the tech sheet (FICHE§6.3), rewritten by
/// `techsheet::refresh_cache` from every source, the rules being one of them.
#[allow(clippy::too_many_arguments)]
pub fn update_harmonization(
    conn: &Connection,
    id: &str,
    brand: Option<&str>,
    car_class: Option<&str>,
    category: Option<&str>,
    categories: &[String],
    country: Option<&str>,
    tags_from_rule: &[String],
) -> rusqlite::Result<()> {
    conn.execute(
        r#"UPDATE mods SET
               brand = COALESCE(?2, brand),
               car_class = ?3,
               category = ?4,
               categories = ?5,
               country = COALESCE(?6, country),
               tags_from_rule = ?7
           WHERE id_interne = ?1"#,
        params![
            id,
            brand,
            car_class,
            category,
            serde_json::to_string(categories).unwrap_or_else(|_| "[]".into()),
            country,
            serde_json::to_string(tags_from_rule).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

/// Renseigne le pack/URL d'origine d'un mod (§4.4). N'écrase une valeur
/// existante que si une nouvelle est fournie (COALESCE) — un ré-import ne
/// perd pas l'URL renseignée par ailleurs.
pub fn set_source(conn: &Connection, id: &str, pack: Option<&str>, url: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET source_pack = COALESCE(?2, source_pack),
                         source_url  = COALESCE(?3, source_url)
         WHERE id_interne = ?1",
        params![id, pack, url],
    )?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET is_favorite = ?2 WHERE id_interne = ?1",
        params![id, fav as i64],
    )?;
    Ok(())
}

pub fn set_manual_tags(conn: &Connection, id: &str, tags: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET tags_manual = ?2 WHERE id_interne = ?1",
        params![id, serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())],
    )?;
    Ok(())
}

/// Édite un champ overlay simple (liste blanche de colonnes pour éviter toute
/// injection). `value = None` met la colonne à NULL.
pub fn set_mod_field(conn: &Connection, id: &str, field: &str, value: Option<&str>) -> rusqlite::Result<()> {
    let column = match field {
        "category" => "category",
        "car_class" => "car_class",
        "country" => "country",
        // Not drivetrain, aspiration, gearbox, engine_config, engine_pos: they
        // are the tech sheet's cache (FICHE§6.3), a write here would be undone
        // at the next refresh. A correction goes through `techsheet::save_user`.
        // Saisies libres de l'utilisateur (§5bis.3) : jamais écrites dans le
        // `ui_*.json` du mod (règle d'or n°1), donc conservées quand l'auteur
        // publie une mise à jour.
        "display_name_user" => "display_name_user",
        "description_user" => "description_user",
        _ => return Err(rusqlite::Error::InvalidParameterName(field.into())),
    };
    conn.execute(
        &format!("UPDATE mods SET {column} = ?2 WHERE id_interne = ?1"),
        params![id, value],
    )?;
    Ok(())
}

pub fn add_history(conn: &Connection, mod_id: &str, ts: &str, event: &str, details: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO history (mod_id, timestamp, event, details) VALUES (?1,?2,?3,?4)",
        params![mod_id, ts, event, details],
    )?;
    Ok(())
}

// --- Lectures ---------------------------------------------------------------

pub(super) const MOD_SELECT: &str = r#"
    SELECT m.id_interne, m.kind, m.brand,
           -- Nom effectif (§5bis.3) : la saisie de l'utilisateur l'emporte sur
           -- ce qu'annonce le `ui_*.json`. Résolu ICI et pas chez l'appelant,
           -- pour que TOUT ce qui affiche un mod en profite d'un coup — liste,
           -- fiche, sélecteur de session, adversaires, export.
           COALESCE(m.display_name_user, m.display_name) AS display_name,
           -- Year and country as the user corrected them on the tech sheet
           -- (FICHE§8): resolved here, like the name, so the index, the
           -- filters and the columns follow the correction without a cache.
           -- A decision is taken whole, a forced "unknown" (NULL) included:
           -- the list must not show a year the sheet no longer does.
           CASE WHEN EXISTS (SELECT 1 FROM tech_user u WHERE u.mod_id = m.id_interne AND u.field = 'year')
                THEN (SELECT json_extract(u.value, '$') FROM tech_user u
                      WHERE u.mod_id = m.id_interne AND u.field = 'year')
                ELSE m.year END AS year,
           m.car_class,
           m.category,
           CASE WHEN EXISTS (SELECT 1 FROM tech_user u WHERE u.mod_id = m.id_interne AND u.field = 'country')
                THEN (SELECT json_extract(u.value, '$') FROM tech_user u
                      WHERE u.mod_id = m.id_interne AND u.field = 'country')
                ELSE m.country END AS country,
           m.is_favorite, m.active_version_id,
           -- Pas de date d'ajout pour le contenu de base : voir ModRow.created_at.
           CASE WHEN m.is_stock THEN NULL ELSE m.created_at END AS created_at,
           m.tags_from_rule, m.tags_manual,
           m.drivetrain, m.engine_pos, m.aspiration, m.engine_config, m.gearbox,
           (SELECT COUNT(*) FROM versions v WHERE v.mod_id = m.id_interne) AS version_count,
           COALESCE((SELECT v.tags_from_mod FROM versions v
                     WHERE v.id = m.active_version_id), '[]') AS tags_from_mod,
           m.source_pack, m.source_url,
           -- Données de la version active (colonnes §6.2) + date de MAJ agrégée.
           (SELECT v.author FROM versions v WHERE v.id = m.active_version_id) AS author,
           (SELECT v.version_label FROM versions v WHERE v.id = m.active_version_id) AS version_label,
           COALESCE((SELECT v.layouts FROM versions v WHERE v.id = m.active_version_id), '[]') AS layouts,
           COALESCE((SELECT v.csp_features FROM versions v WHERE v.id = m.active_version_id), '[]') AS csp_features,
           -- Idem : pas de date de MAJ pour le contenu de base (§ commentaire
           -- de ModRow.created_at) — l'agrégat MAX(imported_at) n'y vaut que
           -- l'instant du réindex, pas une vraie mise à jour.
           CASE WHEN m.is_stock THEN NULL
                ELSE (SELECT MAX(v.imported_at) FROM versions v WHERE v.mod_id = m.id_interne)
           END AS updated_at,
           m.is_stock,
           (SELECT v.published_at FROM versions v WHERE v.id = m.active_version_id) AS published_at,
           (SELECT SUM(v.size_bytes) FROM versions v WHERE v.mod_id = m.id_interne) AS size_bytes,
           m.categories,
           -- Les deux saisies brutes, pour que la fiche sache qu'un nom est
           -- surchargé (et propose de revenir à l'original) — `display_name`
           -- ci-dessus ne le dit plus, par construction.
           m.display_name_user, m.description_user,
           -- Le nom tel que l'annonce le fichier du mod, que `display_name`
           -- ci-dessus masque dès qu'une surcharge existe : c'est pourtant lui
           -- qu'il faut montrer à qui hésite à revenir en arrière.
           m.display_name AS display_name_file,
           m.is_unmanaged,
           m.notes_user,
           m.tech_marks,
           COALESCE((SELECT v.content_state = 'skeleton' FROM versions v
                     WHERE v.id = m.active_version_id), 0) AS showcase
    FROM mods m
"#;

pub(super) fn map_mod(row: &rusqlite::Row) -> rusqlite::Result<ModRow> {
    let tags_rule: String = row.get(11)?;
    let tags_manual: String = row.get(12)?;
    let tags_mod: String = row.get(19)?;
    let layouts: String = row.get(24)?;
    let csp_features: String = row.get(25)?;
    let categories: String = row.get(30)?;
    Ok(ModRow {
        id_interne: row.get(0)?,
        kind: row.get(1)?,
        brand: row.get(2)?,
        display_name: row.get(3)?,
        year: row.get(4)?,
        car_class: row.get(5)?,
        category: row.get(6)?,
        categories: json_arr(&categories),
        country: row.get(7)?,
        is_favorite: row.get::<_, i64>(8)? != 0,
        active_version_id: row.get(9)?,
        created_at: row.get(10)?,
        tags_from_rule: json_arr(&tags_rule),
        tags_manual: json_arr(&tags_manual),
        drivetrain: row.get(13)?,
        engine_pos: row.get(14)?,
        aspiration: row.get(15)?,
        engine_config: row.get(16)?,
        gearbox: row.get(17)?,
        version_count: row.get(18)?,
        tags_from_mod: json_arr(&tags_mod),
        source_pack: row.get(20)?,
        source_url: row.get(21)?,
        author: row.get(22)?,
        active_version_label: row.get(23)?,
        layouts: json_arr(&layouts),
        csp_features: json_arr(&csp_features),
        updated_at: row.get(26)?,
        is_stock: row.get::<_, i64>(27)? != 0,
        published_at: row.get(28)?,
        size_bytes: row.get(29)?,
        display_name_user: row.get(31)?,
        description_user: row.get(32)?,
        display_name_file: row.get(33)?,
        is_unmanaged: row.get::<_, i64>(34)? != 0,
        notes_user: row.get(35)?,
        tech_marks: serde_json::from_str(&row.get::<_, String>(36)?).unwrap_or_default(),
        showcase: row.get::<_, i64>(37)? != 0,
    })
}

pub fn list_mods(conn: &Connection) -> rusqlite::Result<Vec<ModRow>> {
    // Tri sur le nom EFFECTIF : trier sur `m.display_name` rangerait un mod
    // renommé à sa place d'avant, invisible pour qui lit la liste.
    let sql = format!("{MOD_SELECT} ORDER BY COALESCE(m.display_name_user, m.display_name) COLLATE NOCASE");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_mod)?;
    rows.collect()
}

pub fn get_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<ModRow>> {
    let sql = format!("{MOD_SELECT} WHERE m.id_interne = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], map_mod)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Colonnes de `versions`, dans l'ordre attendu par [`version_row`].
pub(super) const VERSION_COLUMNS: &str = r#"id, mod_id, version_label, author, imported_at, library_path,
       source_archive, content_signature, csp_features, skins, layouts, tags_from_mod,
       published_at, size_bytes, kept_archive_path,
       content_state, freed_at, freed_bytes, source_site, source_file_name"#;

pub(super) fn version_row(row: &rusqlite::Row) -> rusqlite::Result<VersionRow> {
    let csp: String = row.get(8)?;
    let skins: String = row.get(9)?;
    let layouts: String = row.get(10)?;
    let tags: String = row.get(11)?;
    Ok(VersionRow {
        id: row.get(0)?,
        mod_id: row.get(1)?,
        version_label: row.get(2)?,
        author: row.get(3)?,
        imported_at: row.get(4)?,
        library_path: row.get(5)?,
        source_archive: row.get(6)?,
        content_signature: row.get(7)?,
        csp_features: json_arr(&csp),
        skins: json_arr(&skins),
        layouts: json_arr(&layouts),
        tags_from_mod: json_arr(&tags),
        published_at: row.get(12)?,
        size_bytes: row.get(13)?,
        kept_archive_path: row.get(14)?,
        content_state: row.get(15)?,
        freed_at: row.get(16)?,
        freed_bytes: row.get(17)?,
        source_site: row.get(18)?,
        source_file_name: row.get(19)?,
    })
}

pub fn get_versions(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<VersionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {VERSION_COLUMNS} FROM versions WHERE mod_id = ?1 ORDER BY imported_at DESC"
    ))?;
    let rows = stmt.query_map([mod_id], version_row)?;
    rows.collect()
}

/// Une version par son id — ce que la suppression d'une version a besoin de
/// lire avant d'effacer quoi que ce soit (§10).
pub fn get_version(conn: &Connection, version_id: &str) -> rusqlite::Result<Option<VersionRow>> {
    let mut stmt = conn.prepare(&format!("SELECT {VERSION_COLUMNS} FROM versions WHERE id = ?1"))?;
    let mut rows = stmt.query_map([version_id], version_row)?;
    rows.next().transpose()
}

/// Retire la ligne d'une version. Les fichiers, eux, sont l'affaire de
/// `maintenance::delete_version` — l'overlay ne touche jamais au disque.
pub fn delete_version(conn: &Connection, version_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM versions WHERE id = ?1", params![version_id])?;
    // What its files said about the car (FICHE§6.1) goes with it.
    conn.execute("DELETE FROM tech_facts WHERE version_id = ?1", params![version_id])?;
    Ok(())
}

/// Noms des profils qui épinglent cette version (§10). Un profil pointant
/// une version effacée activerait dans le vide : la suppression le dit avant,
/// et [`repoint_profile_entries`] le recolle après.
pub fn profiles_using_version(conn: &Connection, version_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT p.name FROM profiles p
         JOIN profile_entries e ON e.profile_id = p.id
         WHERE e.version_id = ?1 ORDER BY p.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([version_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Repointe les profils d'une version vers une autre — appelé à la suppression.
///
/// Repointer plutôt que supprimer l'entrée : sans version, le profil ne
/// contient plus ce mod, donc l'appliquer le **désactiverait** au lieu de
/// l'activer. Le profil perd l'épinglage d'une version précise, jamais son
/// intention.
pub fn repoint_profile_entries(conn: &Connection, from_version: &str, to_version: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE profile_entries SET version_id = ?2 WHERE version_id = ?1",
        params![from_version, to_version],
    )
}

/// Enregistre le chemin de l'archive/dossier source conservé pour une version
/// (§10/§11), copié à l'import quand le réglage `keep_source_archive` est actif.
pub fn set_kept_archive(conn: &Connection, version_id: &str, path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET kept_archive_path = ?1 WHERE id = ?2",
        params![path, version_id],
    )?;
    Ok(())
}

/// Where a version's archive was downloaded from (ESPACE§8.2). Fills what is
/// unknown and never replaces what is known: a rehydration from a copy found
/// elsewhere must not erase the site of the original download.
pub fn set_version_origin(
    conn: &Connection,
    version_id: &str,
    origin: &crate::archive::Origin,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET source_site = COALESCE(source_site, ?2),
                             source_file_name = COALESCE(source_file_name, ?3)
         WHERE id = ?1",
        params![version_id, origin.site, origin.file_name],
    )?;
    Ok(())
}

/// Forgets the kept source of a version, whose files were just removed
/// (ESPACE§5.2: the showcase was asked not to keep it).
pub fn clear_kept_archive(conn: &Connection, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET kept_archive_path = NULL WHERE id = ?1",
        params![version_id],
    )?;
    Ok(())
}

/// Vrai si une version réclame encore cette source conservée (§10/§11).
///
/// Fait autorité pour décider si une copie fraîchement posée dans
/// `_source_archives/` sert à quelque chose : l'import la fait **avant** de
/// savoir si elle sera retenue (doublon, contenu non reconnu, couche… ne
/// stockent aucune version), et sans ce contrôle elle resterait sur le disque
/// sans que rien ne la référence ni ne la nettoie.
pub fn kept_archive_in_use(conn: &Connection, path: &str) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM versions WHERE kept_archive_path = ?1",
        params![path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn get_history(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<HistoryRow>> {
    let mut stmt = conn.prepare("SELECT timestamp, event, details FROM history WHERE mod_id = ?1 ORDER BY id DESC")?;
    let rows = stmt.query_map([mod_id], |row| {
        Ok(HistoryRow {
            timestamp: row.get(0)?,
            event: row.get(1)?,
            details: row.get(2)?,
        })
    })?;
    rows.collect()
}

/// Rapprochement flou : mods de même type ayant le même brand+name normalisé
/// mais un `id_interne` différent (§4.2 « match flou »).
pub fn find_fuzzy(
    conn: &Connection,
    kind: &str,
    brand: &str,
    name: &str,
    exclude_id: &str,
) -> rusqlite::Result<Vec<ModRow>> {
    let sql = format!(
        "{MOD_SELECT} WHERE m.kind = ?1 AND m.id_interne <> ?2
         AND LOWER(TRIM(COALESCE(m.brand,''))) = LOWER(TRIM(?3))
         AND LOWER(TRIM(COALESCE(m.display_name,''))) = LOWER(TRIM(?4))"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![kind, exclude_id, brand, name], map_mod)?;
    rows.collect()
}

/// Signature de contenu de la version active d'un mod (doublon vs mise à jour).
pub fn active_signature(conn: &Connection, mod_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT v.content_signature FROM versions v
         JOIN mods m ON m.active_version_id = v.id WHERE m.id_interne = ?1",
    )?;
    let mut rows = stmt.query_map([mod_id], |r| r.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(r) => Ok(r?),
        None => Ok(None),
    }
}

/// The version of a mod whose content signature is `signature`, a skeleton
/// first (ESPACE§7.3): the comparison runs over **all** versions, since an
/// older one can be the one in the showcase.
pub fn version_by_signature(conn: &Connection, mod_id: &str, signature: &str) -> rusqlite::Result<Option<VersionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {VERSION_COLUMNS} FROM versions WHERE mod_id = ?1 AND content_signature = ?2
         ORDER BY content_state = ?3 DESC, imported_at DESC LIMIT 1"
    ))?;
    let mut rows = stmt.query_map(params![mod_id, signature, CONTENT_SKELETON], version_row)?;
    rows.next().transpose()
}

/// Chemin bibliothèque de la version **active** d'un mod (dossier à comparer à
/// l'entrant pour la détection update/extension, §4.4). `None` si aucune version
/// active (ex. contenu de base sans version bibliothèque).
pub fn active_library_path(conn: &Connection, mod_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT v.library_path FROM versions v
         JOIN mods m ON m.active_version_id = v.id WHERE m.id_interne = ?1",
    )?;
    let mut rows = stmt.query_map([mod_id], |r| r.get::<_, String>(0))?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// The active version of a mod, `None` for a mod without one.
pub fn active_version_id(conn: &Connection, mod_id: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT active_version_id FROM mods WHERE id_interne = ?1",
            [mod_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

/// `content_state` of a version (ESPACE§4.1), `None` for an unknown one.
pub fn version_content_state(conn: &Connection, version_id: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT content_state FROM versions WHERE id = ?1", [version_id], |r| {
        r.get(0)
    })
    .optional()
}

/// Marks a version as a skeleton (ESPACE§5.5, step 5). `size_bytes` is left
/// alone on purpose: it keeps saying how big the mod is — what recovering it
/// will cost, and what the library's size column sorts on —, while the few
/// dozen KB of its skeleton are not worth a column.
pub fn mark_version_freed(
    conn: &Connection,
    version_id: &str,
    freed_at: &str,
    freed_bytes: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET content_state = ?2, freed_at = ?3, freed_bytes = ?4 WHERE id = ?1",
        params![version_id, CONTENT_SKELETON, freed_at, freed_bytes],
    )?;
    Ok(())
}

/// A rehydrated version has its files again (ESPACE§7.3). `freed_*` are
/// cleared with the state: they described a showcase that no longer is.
pub fn mark_version_full(conn: &Connection, version_id: &str, size_bytes: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET content_state = ?2, freed_at = NULL, freed_bytes = NULL, size_bytes = ?3 WHERE id = ?1",
        params![version_id, CONTENT_FULL, size_bytes],
    )?;
    Ok(())
}

/// Chemin bibliothèque d'une version donnée (pour calculer la preview).
pub fn get_version_path(conn: &Connection, version_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT library_path FROM versions WHERE id = ?1")?;
    let mut rows = stmt.query_map([version_id], |r| r.get::<_, String>(0))?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Deletes a mod from the overlay — the **complete** deletion (ESPACE§5.3);
/// the showcase removes no row at all. Files are the caller's business.
///
/// Goes with it, explicitly since these tables have no foreign key: its
/// history, its additions to the game, the tech sheet's corrections, the
/// media the user attached by hand (`media_links`) and the import journal
/// (`import_decisions`). Versions and tech sheet facts cascade.
///
/// Ce qui **survit volontairement** :
///
/// - `usage` (§6) — le marqueur « déjà essayé » et le nombre de lancements.
///   Réimporter la même voiture retrouve son historique d'usage plutôt que de
///   repartir de zéro. Le kilométrage, lui, n'a jamais été chez nous : il vit
///   dans le journal de sessions de Content Manager.
/// - `sub_mods` — skins et sons rattachés, dont les fichiers ne sont pas
///   effacés non plus. Réimporter le parent sous le même id les retrouve
///   automatiquement, ce qui est précisément le geste d'une réinstallation.
///   Ce n'est un déchet que si le parent ne revient jamais : d'où
///   `orphan_subs`, listé en maintenance et nettoyé sur décision.
///
/// - `wiki_link` — the Wikipedia pairing, without a foreign key for this
///   very reason (see the schema).
///
/// Ces tables sont donc absentes de ce `DELETE` **par choix**, pas par
/// oubli — c'est ce que ce commentaire est là pour dire au prochain lecteur.
pub fn delete_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM history WHERE mod_id = ?1", [id])?;
    conn.execute("DELETE FROM media_links WHERE entity_id = ?1", [id])?;
    conn.execute("DELETE FROM import_decisions WHERE mod_id = ?1", [id])?;
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    // The tech sheet's corrections have no foreign key, on purpose (FICHE§6.1):
    // a complete deletion is the one gesture that removes them.
    conn.execute("DELETE FROM tech_user WHERE mod_id = ?1", [id])?;
    conn.execute("DELETE FROM mods WHERE id_interne = ?1", [id])?;
    Ok(())
}
