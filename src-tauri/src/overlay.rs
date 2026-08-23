//! Base d'overlay (§3.0) — SQLite. Source de vérité des **métadonnées produites
//! par l'app** (jamais des fichiers du mod). Indexée sur `id_interne` du mod.
//!
//! L1 peuple : mods, versions (avec snapshot lecture seule des tags du fichier,
//! features CSP, skins, layouts), historique. Les colonnes overlay-éditables
//! (car_class, year, category, is_favorite, tags règle/manuel) existent dès
//! maintenant mais seront pleinement exploitées en L2.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// État partagé Tauri : connexion SQLite protégée par un mutex.
pub struct Db(pub Mutex<Connection>);

/// Version of the harmonisation engine that produced the stored overlay (§5).
pub const META_ENGINE_VERSION: &str = "engine_version";

/// Reads a `meta` entry; `None` when the key was never written.
pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Ajoute les colonnes L2 aux bases déjà créées en L1 (ALTER idempotent).
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let cols = [
        "country TEXT",
        "tags_from_rule TEXT NOT NULL DEFAULT '[]'",
        "tags_manual TEXT NOT NULL DEFAULT '[]'",
        "drivetrain TEXT",
        "engine_pos TEXT",
        "aspiration TEXT",
        "engine_config TEXT",
        "gearbox TEXT",
        "source_pack TEXT",
        "source_url TEXT",
        "is_stock INTEGER NOT NULL DEFAULT 0",
        "categories TEXT NOT NULL DEFAULT '[]'",
        // Nom/description saisis par l'utilisateur (§5bis.3).
        "display_name_user TEXT",
        "description_user TEXT",
        // Mod installé hors Pit Box, trouvé dans content/ à l'indexation (§12bis.1bis).
        "is_unmanaged INTEGER NOT NULL DEFAULT 0",
    ];
    for col in cols {
        // Ignore l'erreur « duplicate column » si la colonne existe déjà.
        let _ = conn.execute(&format!("ALTER TABLE mods ADD COLUMN {col}"), []);
    }
    // Date de publication estimée depuis les dates de fichiers (§6.2).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN published_at TEXT", []);
    // Taille sur disque de la version, octets (§9.4).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN size_bytes INTEGER", []);
    // Archive/dossier source conservé (§10/§11), si le réglage était activé à
    // l'import de cette version. Rend possible « Réinstaller depuis l'archive
    // source ». `NULL` = non conservé (comportement par défaut).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN kept_archive_path TEXT", []);
    // Couches/extensions (§4.4) : état actif (par défaut) + ordre de priorité.
    let _ = conn.execute("ALTER TABLE layers ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1", []);
    let _ = conn.execute("ALTER TABLE layers ADD COLUMN priority INTEGER NOT NULL DEFAULT 0", []);
    // Skin fourni avec le contenu initial du mod (découvert sur disque, jamais
    // importé séparément par Pit Box) → non supprimable individuellement,
    // seulement le mod entier (§8, même logique que les skins voiture).
    // Défaut 1 (supprimable) pour tous les sous-éléments existants/normaux.
    let _ = conn.execute(
        "ALTER TABLE sub_mods ADD COLUMN removable INTEGER NOT NULL DEFAULT 1",
        [],
    );
    Ok(())
}

fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS mods (
            id_interne        TEXT PRIMARY KEY,
            kind              TEXT NOT NULL,          -- 'Car' | 'Track'
            brand             TEXT,
            display_name      TEXT,
            -- Nom et description saisis par l'utilisateur (§5bis.3). Séparés des
            -- champs dérivés du `ui_*.json` juste au-dessus : ceux-là sont
            -- rafraîchis à chaque réindex/mise à jour du mod, une saisie qui y
            -- vivrait serait écrasée à la première mise à jour de l'auteur.
            display_name_user TEXT,
            description_user  TEXT,
            identity_hash     TEXT,
            car_class         TEXT,                   -- overlay-éditable (L2)
            year              INTEGER,
            category          TEXT,                   -- tag # principal (§5bis)
            categories        TEXT NOT NULL DEFAULT '[]', -- catégories circuit multi-valué (§5bis.2)
            country           TEXT,
            is_favorite       INTEGER NOT NULL DEFAULT 0,
            tags_from_rule    TEXT NOT NULL DEFAULT '[]',
            tags_manual       TEXT NOT NULL DEFAULT '[]',
            drivetrain        TEXT,
            engine_pos        TEXT,
            aspiration        TEXT,
            engine_config     TEXT,
            gearbox           TEXT,
            source_pack       TEXT,                   -- pack d'origine (§4.4)
            source_url        TEXT,                   -- URL d'origine (§4.4/§12ter)
            is_stock          INTEGER NOT NULL DEFAULT 0, -- indexé depuis content/ (§12bis.1)
            is_unmanaged      INTEGER NOT NULL DEFAULT 0, -- ... et pas du Kunos (§12bis.1bis)
            active_version_id TEXT,
            created_at        TEXT NOT NULL
        );

        -- Sous-éléments rattachés à une voiture/circuit (§12bis.2) : skins, sons.
        -- Ne polluent jamais la bibliothèque principale (mods de 1er niveau).
        CREATE TABLE IF NOT EXISTS sub_mods (
            id             TEXT PRIMARY KEY,
            sub_type       TEXT NOT NULL,          -- 'SKIN'|'SOUND'|'TRACK_SKIN'|'TRACK_MOD'
            parent_id      TEXT NOT NULL,          -- id_interne de la voiture/circuit cible (mod OU stock)
            name           TEXT NOT NULL,
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            is_active      INTEGER NOT NULL DEFAULT 0, -- SOUND (exclusif) et TRACK_SKIN (pas exclusif)
            removable      INTEGER NOT NULL DEFAULT 1, -- faux si fourni avec le mod, découvert sur disque
            imported_at    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sub_parent ON sub_mods(parent_id);

        -- Apps Python (§12bis.4) : type autonome, activable par junction.
        CREATE TABLE IF NOT EXISTS apps (
            id             TEXT PRIMARY KEY,       -- nom du dossier de l'app
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            imported_at    TEXT NOT NULL
        );

        -- Suivi d'usage propre à l'app (§6.5) : marqueur « déjà essayé » définitif
        -- posé au lancement d'une session. Fiabilise les faux zéros de CM.
        CREATE TABLE IF NOT EXISTS usage (
            mod_id        TEXT PRIMARY KEY,  -- id_interne de la voiture ou du circuit
            launched      INTEGER NOT NULL DEFAULT 0,
            launch_count  INTEGER NOT NULL DEFAULT 0,
            last_launched TEXT
        );

        CREATE TABLE IF NOT EXISTS versions (
            id                TEXT PRIMARY KEY,
            mod_id            TEXT NOT NULL REFERENCES mods(id_interne) ON DELETE CASCADE,
            version_label     TEXT,
            author            TEXT,
            imported_at       TEXT NOT NULL,
            library_path      TEXT NOT NULL,
            source_archive    TEXT,
            content_signature TEXT,
            csp_features      TEXT NOT NULL DEFAULT '[]',
            skins             TEXT NOT NULL DEFAULT '[]',
            layouts           TEXT NOT NULL DEFAULT '[]',
            tags_from_mod     TEXT NOT NULL DEFAULT '[]',
            published_at      TEXT,                   -- date de publication estimée (§6.2)
            size_bytes        INTEGER                 -- taille sur disque, octets (§9.4)
        );

        CREATE TABLE IF NOT EXISTS history (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            mod_id    TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            event     TEXT NOT NULL,
            details   TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS profiles (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_entries (
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            mod_id     TEXT NOT NULL,
            version_id TEXT NOT NULL
        );

        -- Autres mods (§7.3) et Apps (§12bis.4) capturés par un profil : ni
        -- l'un ni l'autre n'a de notion de version (juste actif/inactif), donc
        -- une table séparée plutôt que de rendre `version_id` optionnelle sur
        -- profile_entries (SQLite ne sait pas assouplir une contrainte NOT NULL
        -- par ALTER). `kind` distingue 'other' | 'app', `entry_id` est l'id dans
        -- la table other_mods ou apps selon le cas.
        CREATE TABLE IF NOT EXISTS profile_extra_entries (
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            kind       TEXT NOT NULL,
            entry_id   TEXT NOT NULL
        );

        -- Mods « autres » (§7.3) : ni voiture, circuit, skin, son, ni app —
        -- jamais perdus. Activables par junction (garde-fou habituel) ; en cas
        -- d'emplacement disputé avec un autre mod « autre », la priorité tranche.
        CREATE TABLE IF NOT EXISTS other_mods (
            id             TEXT PRIMARY KEY,
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            imported_at    TEXT NOT NULL,
            is_priority    INTEGER NOT NULL DEFAULT 0,
            is_active      INTEGER NOT NULL DEFAULT 0,
            junctions      TEXT NOT NULL DEFAULT '[]'
        );

        -- Couches / extensions (§4.4) : contenu importé par-dessus une base
        -- (mod OU stock) qui n'est PAS une mise à jour — surtout des chemins
        -- nouveaux. Rangé à part, ne touche jamais la base (jamais destructif).
        CREATE TABLE IF NOT EXISTS layers (
            id                TEXT PRIMARY KEY,
            parent_id         TEXT NOT NULL,          -- id_interne de la base (mod ou stock)
            parent_kind       TEXT NOT NULL,          -- 'Car' | 'Track'
            name              TEXT NOT NULL,          -- nom de l'archive/dossier source
            library_path      TEXT NOT NULL,          -- <lib>/layers/<parent_id>/<name>
            source_archive    TEXT,
            added_count       INTEGER NOT NULL DEFAULT 0,
            overwritten_count INTEGER NOT NULL DEFAULT 0,
            is_active         INTEGER NOT NULL DEFAULT 1, -- appliquée à la composition (§4.4)
            priority          INTEGER NOT NULL DEFAULT 0, -- ordre : la + haute gagne
            imported_at       TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_layers_parent ON layers(parent_id);

        -- Rattachement manuel d'un screenshot/replay (§6.1) : repli quand le
        -- matching automatique par nom de fichier (media.rs) ne trouve pas
        -- l'entité, ou pour corriger un faux négatif. Jamais rempli par le
        -- matching automatique lui-même — uniquement par une action explicite
        -- « Associer un fichier » côté fiche.
        CREATE TABLE IF NOT EXISTS media_links (
            file_path TEXT NOT NULL,
            entity_id TEXT NOT NULL, -- id_interne voiture/circuit
            kind      TEXT NOT NULL, -- 'SCREENSHOT' | 'REPLAY'
            PRIMARY KEY (file_path, entity_id)
        );
        CREATE INDEX IF NOT EXISTS idx_media_links_entity ON media_links(entity_id);

        -- Ajouts au jeu posés dans AC pour un mod (§4.5.3) : ce qui a été
        -- réellement écrit hors de `content/<type>/<id>` à la dernière
        -- activation. Retirer exactement cette liste — et rien d'autre — est ce
        -- qui rend la désinstallation propre : un fichier qu'on n'a pas posé
        -- (contenu Kunos, ajout d'un autre mod) n'y figure jamais.
        -- `is_dir` : dossier créé pour l'occasion, à élaguer au retrait. Sans
        -- cette distinction, l'élagage se fondait sur « dossier vide » et
        -- pouvait emporter un dossier d'AC préexistant devenu vide.
        -- `kind`/`claimed_at` sont dupliqués depuis `mods` **volontairement** :
        -- une ligne doit suffire à elle-même pour décider quoi poser. Avec une
        -- jointure, une ligne `mods` manquante faisait disparaître la
        -- réclamation, et l'arbitrage effaçait d'AC un fichier encore utile.
        -- `provided` : c'est *cette* ligne qui fournit l'exemplaire actuellement
        -- posé dans AC (au plus une par chemin). Sans elle, il faudrait déduire
        -- le fournisseur de la taille et de la date du fichier posé — ce qui
        -- échoue précisément dans le cas qu'on veut arbitrer, deux exemplaires
        -- de même date (archives repackées).
        CREATE TABLE IF NOT EXISTS extra_links (
            mod_id     TEXT NOT NULL,
            ac_path    TEXT NOT NULL,
            is_dir     INTEGER NOT NULL DEFAULT 0,
            kind       TEXT NOT NULL DEFAULT 'Car',
            claimed_at TEXT NOT NULL DEFAULT '',
            provided   INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (mod_id, ac_path)
        );
        CREATE INDEX IF NOT EXISTS idx_sat_path ON extra_links(ac_path);

        -- Fichiers du jeu qu'un mod a remplacés (§4.5.4), et où dort l'original.
        -- Clé sur le chemin d'AC, pas sur le mod : c'est le fichier qui n'a
        -- qu'un seul original, quel que soit le nombre de mods qui le visent.
        CREATE TABLE IF NOT EXISTS game_backups (
            ac_path     TEXT PRIMARY KEY,
            backup_path TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT ''
        );

        -- Journal des décisions d'import (§4.6). L'app tranche seule tout ce
        -- qui est déterminable depuis le disque — c'est son travail — mais une
        -- décision fausse et **silencieuse** est ce qui a coûté le plus cher :
        -- un pilote posé au mauvais endroit et trois dossiers d'emballage
        -- déversés à la racine du jeu sont restés invisibles jusqu'à ce qu'on
        -- aille lire le disque à la main. Ce journal est la trace lisible de
        -- ces arbitrages, consultable longtemps après l'import.
        --
        -- `mod_id` est nullable : une décision peut concerner un reste qu'aucun
        -- mod ne réclame. Pas de clé étrangère pour la même raison, et pour que
        -- la suppression d'un mod n'efface pas l'explication de ce qu'il a
        -- laissé derrière lui.
        -- Chemins d'AC que l'utilisateur a **explicitement** demande d'installer
        -- (§4.6ter). L'arbitrage par date (§4.5.4) protege les poses
        -- automatiques : il empeche un exemplaire plus ancien de deloger ce qui
        -- tourne. Il n'a aucune autorite contre une decision prise en connaissance
        -- de cause — l'utilisateur venait de lire « remplace N fichiers du jeu de
        -- base » et a repondu « ajouter au jeu ».
        --
        -- Table separee de `extra_links`, qui est effacee et reecrite a chaque
        -- deploiement : l'autorisation, elle, doit survivre a une desactivation
        -- suivie d'une reactivation. La sauvegarde de l'original reste
        -- obligatoire — seule la comparaison de dates est levee.
        CREATE TABLE IF NOT EXISTS forced_extras (
            mod_id  TEXT NOT NULL,
            ac_path TEXT NOT NULL,
            PRIMARY KEY (mod_id, ac_path)
        );

        -- Dossiers proposes par l'auteur (§4.6ter) : livres a cote du mod, ni
        -- chemin de jeu ni annexe, donc sans sort deductible du disque. Ranges
        -- en attente et **jamais poses** tant que l'utilisateur n'a pas
        -- tranche. La ligne survit a un redemarrage : la question est posee en
        -- fin de lot, mais ne rien decider est une reponse valable, et ce qui
        -- attend ne doit pas disparaitre parce qu'on a ferme l'app.
        CREATE TABLE IF NOT EXISTS pending_folders (
            id           TEXT PRIMARY KEY,
            archive      TEXT NOT NULL DEFAULT '',
            rel_path     TEXT NOT NULL,
            library_path TEXT NOT NULL,
            owner_id     TEXT,
            owner_kind   TEXT,
            shape        TEXT NOT NULL DEFAULT 'unknown',
            title        TEXT,
            description  TEXT,
            readme       TEXT,
            skin_target  TEXT,
            replaced     INTEGER NOT NULL DEFAULT 0,
            found_at     TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS import_decisions (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            mod_id     TEXT,
            archive    TEXT NOT NULL DEFAULT '',
            kind       TEXT NOT NULL,
            subject    TEXT NOT NULL,
            detail     TEXT,
            decided_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_decisions_mod ON import_decisions(mod_id);

        -- Petit magasin clé/valeur décrivant la base elle-même, par
        -- opposition à ce qu'elle contient (§5 : version du moteur qui a
        -- calculé l'harmonisation stockée). Volontairement pas dans
        -- `config.json` : le frontend le réécrit en entier, un marqueur qu'il
        -- ignore y disparaîtrait au premier enregistrement des réglages.
        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_versions_mod ON versions(mod_id);
        CREATE INDEX IF NOT EXISTS idx_history_mod  ON history(mod_id);
        CREATE INDEX IF NOT EXISTS idx_mods_idhash  ON mods(identity_hash);
        CREATE INDEX IF NOT EXISTS idx_pe_profile   ON profile_entries(profile_id);
        CREATE INDEX IF NOT EXISTS idx_pee_profile  ON profile_extra_entries(profile_id);
        "#,
    )
}

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
    /// Catégories de circuit (§5bis.2), multi-valué, ordonnées par priorité.
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
    /// URL d'origine (rempli plus tard par l'extension, §4.4/§12ter).
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
    /// bibliothèque — lecture seule, non désactivable (§12bis.1). Vrai pour le
    /// contenu de base Kunos **comme** pour un mod installé hors Pit Box :
    /// c'est ce qui fait que tout ce qui protège l'un protège l'autre (lecture
    /// des vignettes dans `content/`, refus d'activation, absence de date
    /// d'ajout…). La distinction se lit sur `is_unmanaged`.
    pub is_stock: bool,
    /// Mod installé hors Pit Box (§12bis.1bis) : présent dans `content/` comme
    /// un vrai dossier, mais absent de la table du contenu officiel
    /// ([`crate::kunos_dates::is_official`]). Toujours accompagné de
    /// `is_stock`. Contrairement au contenu de base, ce n'est **pas** du
    /// contenu de jeu : il ne reçoit ni couche, ni import par-dessus, et il
    /// n'est jamais sauvegardé/effacé de `content/` — l'app le laisse
    /// strictement où l'utilisateur l'a mis, définitivement. Le faire passer
    /// sous gestion suppose que l'utilisateur retire lui-même le dossier du
    /// jeu et importe le mod.
    pub is_unmanaged: bool,
    /// Date de publication estimée de la version active (§6.2).
    pub published_at: Option<String>,
    /// Taille sur disque cumulée de toutes les versions, octets (§9.4).
    /// `None` tant qu'aucune n'a été calculée (mod importé avant cette
    /// fonctionnalité, à rattraper via « Réindexer » + recalcul de taille).
    pub size_bytes: Option<i64>,
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
    /// Taille sur disque de cette version, octets (§9.4).
    pub size_bytes: Option<i64>,
    /// Archive/dossier source conservé en bibliothèque (§10/§11), si le
    /// réglage était activé à l'import. `None` = non conservé.
    pub kept_archive_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub timestamp: String,
    pub event: String,
    pub details: String,
}

fn json_arr(s: &str) -> Vec<String> {
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

/// Renseigne la taille sur disque d'une version, octets (§9.4). Séparé de
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

/// Écrit le résultat d'harmonisation (§5.4) dans l'overlay. brand/country et les
/// specs ne sont écrasés que si une valeur est fournie (préserve les complétions
/// manuelles) ; tags_from_rule/car_class/category reflètent toujours les règles.
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
    drivetrain: Option<&str>,
    engine_pos: Option<&str>,
    aspiration: Option<&str>,
    engine_config: Option<&str>,
    gearbox: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"UPDATE mods SET
               brand = COALESCE(?2, brand),
               car_class = ?3,
               category = ?4,
               categories = ?5,
               country = COALESCE(?6, country),
               tags_from_rule = ?7,
               drivetrain = COALESCE(?8, drivetrain),
               engine_pos = COALESCE(?9, engine_pos),
               aspiration = COALESCE(?10, aspiration),
               engine_config = COALESCE(?11, engine_config),
               gearbox = COALESCE(?12, gearbox)
           WHERE id_interne = ?1"#,
        params![
            id,
            brand,
            car_class,
            category,
            serde_json::to_string(categories).unwrap_or_else(|_| "[]".into()),
            country,
            serde_json::to_string(tags_from_rule).unwrap_or_else(|_| "[]".into()),
            drivetrain,
            engine_pos,
            aspiration,
            engine_config,
            gearbox,
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
        "drivetrain" => "drivetrain",
        "engine_pos" => "engine_pos",
        "aspiration" => "aspiration",
        "engine_config" => "engine_config",
        "gearbox" => "gearbox",
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

const MOD_SELECT: &str = r#"
    SELECT m.id_interne, m.kind, m.brand,
           -- Nom effectif (§5bis.3) : la saisie de l'utilisateur l'emporte sur
           -- ce qu'annonce le `ui_*.json`. Résolu ICI et pas chez l'appelant,
           -- pour que TOUT ce qui affiche un mod en profite d'un coup — liste,
           -- fiche, sélecteur de session, adversaires, export.
           COALESCE(m.display_name_user, m.display_name) AS display_name,
           m.year, m.car_class,
           m.category, m.country, m.is_favorite, m.active_version_id,
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
           m.is_unmanaged
    FROM mods m
"#;

fn map_mod(row: &rusqlite::Row) -> rusqlite::Result<ModRow> {
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
const VERSION_COLUMNS: &str = r#"id, mod_id, version_label, author, imported_at, library_path,
       source_archive, content_signature, csp_features, skins, layouts, tags_from_mod,
       published_at, size_bytes, kept_archive_path"#;

fn version_row(row: &rusqlite::Row) -> rusqlite::Result<VersionRow> {
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

/// Chemin bibliothèque d'une version donnée (pour calculer la preview).
pub fn get_version_path(conn: &Connection, version_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT library_path FROM versions WHERE id = ?1")?;
    let mut rows = stmt.query_map([version_id], |r| r.get::<_, String>(0))?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Supprime un mod et ses données overlay (versions cascade + historique).
/// N'agit que sur l'overlay : les fichiers bibliothèque sont gérés par l'appelant.
/// Supprime un mod de l'overlay. Ce qui **survit volontairement** :
///
/// - `usage` (§6.5) — le marqueur « déjà essayé » et le nombre de lancements.
///   Réimporter la même voiture retrouve son historique d'usage plutôt que de
///   repartir de zéro. Le kilométrage, lui, n'a jamais été chez nous : il vit
///   dans le journal de sessions de Content Manager.
/// - `sub_mods` — skins et sons rattachés, dont les fichiers ne sont pas
///   effacés non plus. Réimporter le parent sous le même id les retrouve
///   automatiquement, ce qui est précisément le geste d'une réinstallation.
///   Ce n'est un déchet que si le parent ne revient jamais : d'où
///   `orphan_subs`, listé en maintenance et nettoyé sur décision.
///
/// Les deux tables sont donc absentes de ce `DELETE` **par choix**, pas par
/// oubli — c'est ce que ce commentaire est là pour dire au prochain lecteur.
pub fn delete_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM history WHERE mod_id = ?1", [id])?;
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    conn.execute("DELETE FROM mods WHERE id_interne = ?1", [id])?;
    Ok(())
}

// --- Fichiers du jeu remplacés (§4.5.4) ---------------------------------------

/// Enregistre la sauvegarde de l'original. `INSERT OR IGNORE` : la **première**
/// sauvegarde fait foi, un second mod visant le même chemin ne l'écrase pas.
pub fn add_game_backup(conn: &Connection, ac_path: &str, backup_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO game_backups (ac_path, backup_path, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![ac_path, backup_path, chrono::Local::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn game_backup_of(conn: &Connection, ac_path: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT backup_path FROM game_backups WHERE ac_path = ?1")?;
    let mut rows = stmt.query_map([ac_path], |r| r.get::<_, String>(0))?;
    rows.next().transpose()
}

pub fn remove_game_backup(conn: &Connection, ac_path: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM game_backups WHERE ac_path = ?1", [ac_path])?;
    Ok(())
}

pub fn list_game_backups(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT ac_path, backup_path FROM game_backups")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect()
}

// --- Ajouts au jeu posés dans AC (§4.5.3) ----------------------------------

/// Remplace la liste des ajouts posés pour un mod (liste vide = plus rien
/// de posé). Réécriture complète : c'est l'état du disque après l'opération qui
/// est mémorisé, jamais un cumul. `(chemin, est_un_dossier_créé)`.
pub fn set_extra_links(
    conn: &Connection,
    mod_id: &str,
    kind: &str,
    entries: &[(String, bool)],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [mod_id])?;
    let now = chrono::Local::now().to_rfc3339();
    for (p, is_dir) in entries {
        conn.execute(
            "INSERT OR IGNORE INTO extra_links (mod_id, ac_path, is_dir, kind, claimed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![mod_id, p, *is_dir as i64, kind, now],
        )?;
    }
    Ok(())
}

/// Ce qu'un mod réclame — `(ac_path, is_dir, kind)`. Le `kind` est celui qu'a
/// écrit [`set_extra_links`], donc la forme `content_folder()` ("cars"/"tracks")
/// : il faut le rendre avec le reste, parce que le retrait doit retrouver
/// l'exemplaire en bibliothèque **avant** d'effacer la réclamation — après, plus
/// rien en base ne dit de quel arbre il venait.
pub fn get_extra_links(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<(String, bool, String)>> {
    let mut stmt = conn.prepare("SELECT ac_path, is_dir, kind FROM extra_links WHERE mod_id = ?1")?;
    let rows = stmt.query_map([mod_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0, r.get::<_, String>(2)?))
    })?;
    rows.collect()
}

/// Mods qui réclament ce fichier d'AC — `(mod_id, kind, claimed_at)`. C'est le
/// compteur de références des fichiers partagés (§4.5.4) : tant qu'il reste au
/// moins une ligne, le fichier est encore réclamé et ne doit pas être retiré
/// d'AC. `claimed_at` départage deux exemplaires de même date de modification :
/// le dernier mod installé gagne.
pub fn extra_claimants(conn: &Connection, ac_path: &str) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut stmt =
        conn.prepare("SELECT mod_id, kind, claimed_at FROM extra_links WHERE ac_path = ?1 AND is_dir = 0")?;
    let rows = stmt.query_map([ac_path], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    rows.collect()
}

/// Une décision prise seule par l'**app** pendant un import (§4.6).
///
/// À ne pas confondre avec `importer::ImportDecision`, qui est la décision de
/// l'**utilisateur** sur un cas ambigu (§4.4). Les deux existent parce que la
/// ligne entre elles est le vrai choix de conception : l'app tranche tout ce
/// qui est déterminable depuis le disque et en **rend compte** ici ; elle ne
/// demande que ce dont la réponse est dans la tête de l'utilisateur.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportJournalEntry {
    /// Clé i18n courte (`pathNormalized`, `pathRefused`, `leftoverAttached`…) —
    /// jamais une phrase : le libellé appartient au frontend.
    pub kind: String,
    /// Ce sur quoi la décision a porté : le chemin du reste, tel qu'il était
    /// dans l'archive.
    pub subject: String,
    /// Ce qui en a été fait — destination, mod de rattachement. `None` quand la
    /// nature de la décision se suffit à elle-même.
    pub detail: Option<String>,
    pub archive: String,
    pub decided_at: String,
}

/// Enregistre une décision. **Best-effort assumé** : le journal explique
/// l'import, il ne le conditionne pas — un échec d'écriture ne doit jamais
/// faire échouer un rangement qui, lui, a réussi.
pub fn record_decision(
    conn: &Connection,
    mod_id: Option<&str>,
    archive: &str,
    kind: &str,
    subject: &str,
    detail: Option<&str>,
) {
    let now = chrono::Local::now().to_rfc3339();
    if let Err(e) = conn.execute(
        "INSERT INTO import_decisions (mod_id, archive, kind, subject, detail, decided_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![mod_id, archive, kind, subject, detail, now],
    ) {
        log::warn!("record_decision {kind} {subject}: {e}");
    }
}

/// Décisions rattachées à un mod, les plus récentes d'abord — c'est le dernier
/// import qui intéresse, pas le premier.
pub fn decisions_for_mod(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<ImportJournalEntry>> {
    let mut stmt = conn.prepare(
        "SELECT kind, subject, detail, archive, decided_at FROM import_decisions
         WHERE mod_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([mod_id], |r| {
        Ok(ImportJournalEntry {
            kind: r.get(0)?,
            subject: r.get(1)?,
            detail: r.get(2)?,
            archive: r.get(3)?,
            decided_at: r.get(4)?,
        })
    })?;
    rows.collect()
}

/// Efface les décisions d'un mod avant de réenregistrer celles d'un nouvel
/// import : sans ça, réimporter un mod corrigé laisserait à l'écran
/// l'explication de l'import fautif, indéfiniment.
pub fn clear_decisions(conn: &Connection, mod_id: &str) {
    if let Err(e) = conn.execute("DELETE FROM import_decisions WHERE mod_id = ?1", [mod_id]) {
        log::warn!("clear_decisions {mod_id}: {e}");
    }
}

/// Efface les décisions d'une archive avant de rejouer son balayage.
///
/// Le nettoyage par mod ne suffit pas : réimporter une archive **à l'identique**
/// classe ses mods en doublons, ce qui court-circuite leur écriture overlay —
/// mais le balayage des restes, lui, tourne quand même et réenregistre tout.
/// Sans ce second nettoyage, chaque réimport empilait un exemplaire de plus de
/// la même décision. C'est aussi le seul moyen d'oublier les décisions qui ne
/// se rattachent à aucun mod (`mod_id` nul).
pub fn clear_decisions_for_archive(conn: &Connection, archive: &str) {
    if let Err(e) = conn.execute("DELETE FROM import_decisions WHERE archive = ?1", [archive]) {
        log::warn!("clear_decisions_for_archive {archive}: {e}");
    }
}

/// Mod dont l'exemplaire est actuellement posé dans AC à ce chemin.
pub fn extra_provider(conn: &Connection, ac_path: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT mod_id FROM extra_links WHERE ac_path = ?1 AND provided = 1")?;
    let mut rows = stmt.query_map([ac_path], |r| r.get::<_, String>(0))?;
    rows.next().transpose()
}

/// Désigne le mod qui fournit désormais ce chemin — au plus un à la fois.
pub fn set_extra_provider(conn: &Connection, ac_path: &str, mod_id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE extra_links SET provided = 0 WHERE ac_path = ?1", [ac_path])?;
    conn.execute(
        "UPDATE extra_links SET provided = 1 WHERE ac_path = ?1 AND mod_id = ?2",
        [ac_path, mod_id],
    )?;
    Ok(())
}

// --- Profils (L3) -----------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileEntry {
    pub mod_id: String,
    pub version_id: String,
}

/// Entrée de profil sans notion de version — Autre mod ou App (§7.3/§12bis.4),
/// simplement actif ou non. `kind` vaut "other" ou "app", `entry_id` est l'id
/// dans la table correspondante.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileExtraEntry {
    pub kind: String,
    pub entry_id: String,
}

pub fn create_profile(conn: &Connection, id: &str, name: &str, created_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, created_at],
    )?;
    Ok(())
}

pub fn add_profile_entry(conn: &Connection, profile_id: &str, mod_id: &str, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profile_entries (profile_id, mod_id, version_id) VALUES (?1, ?2, ?3)",
        params![profile_id, mod_id, version_id],
    )?;
    Ok(())
}

pub fn add_profile_extra_entry(
    conn: &Connection,
    profile_id: &str,
    kind: &str,
    entry_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profile_extra_entries (profile_id, kind, entry_id) VALUES (?1, ?2, ?3)",
        params![profile_id, kind, entry_id],
    )?;
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProfileRow>> {
    let mut stmt = conn.prepare(
        r#"SELECT p.id, p.name,
                  (SELECT COUNT(*) FROM profile_entries e WHERE e.profile_id = p.id)
                  + (SELECT COUNT(*) FROM profile_extra_entries x WHERE x.profile_id = p.id) AS entry_count
           FROM profiles p ORDER BY p.name COLLATE NOCASE"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProfileRow {
            id: r.get(0)?,
            name: r.get(1)?,
            entry_count: r.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn get_profile_entries(conn: &Connection, profile_id: &str) -> rusqlite::Result<Vec<ProfileEntry>> {
    let mut stmt = conn.prepare("SELECT mod_id, version_id FROM profile_entries WHERE profile_id = ?1")?;
    let rows = stmt.query_map([profile_id], |r| {
        Ok(ProfileEntry {
            mod_id: r.get(0)?,
            version_id: r.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn get_profile_extra_entries(conn: &Connection, profile_id: &str) -> rusqlite::Result<Vec<ProfileExtraEntry>> {
    let mut stmt = conn.prepare("SELECT kind, entry_id FROM profile_extra_entries WHERE profile_id = ?1")?;
    let rows = stmt.query_map([profile_id], |r| {
        Ok(ProfileExtraEntry {
            kind: r.get(0)?,
            entry_id: r.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn delete_profile(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
    Ok(())
}

/// Ids des mods partageant un même pack d'origine (§4.4).
pub fn list_pack_ids(conn: &Connection, pack: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id_interne FROM mods WHERE source_pack = ?1")?;
    let rows = stmt.query_map([pack], |r| r.get::<_, String>(0))?;
    rows.collect()
}

pub fn mod_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM mods WHERE id_interne = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

/// Tous les id_interne d'un type (voiture/circuit, mod comme stock) — sert à
/// `media.rs` pour retrouver le « contrepartie » (circuit dans un nom de
/// screenshot de voiture, et inversement, §6.1).
pub fn list_mod_ids_by_kind(conn: &Connection, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id_interne FROM mods WHERE kind = ?1")?;
    let rows = stmt.query_map([kind], |r| r.get::<_, String>(0))?;
    rows.collect()
}

// --- Rattachement manuel de médias (§6.1) -----------------------------------

/// Associe manuellement un fichier (screenshot/replay) à une entité — repli
/// quand `media.rs` ne l'a pas trouvé automatiquement. Idempotent (clé
/// primaire `(file_path, entity_id)`).
pub fn add_media_link(conn: &Connection, file_path: &str, entity_id: &str, kind: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO media_links (file_path, entity_id, kind) VALUES (?1, ?2, ?3)",
        params![file_path, entity_id, kind],
    )?;
    Ok(())
}

/// Retire tout rattachement pointant sur ce fichier, quelle que soit l'entité —
/// appelé quand le fichier part à la corbeille (§6.1). Sans ça, la ligne
/// survivrait au fichier et `merge_*_links` referait apparaître le média
/// supprimé dans la galerie à la prochaine ouverture de l'onglet.
pub fn remove_media_links_for_path(conn: &Connection, file_path: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM media_links WHERE file_path = ?1", params![file_path])
}

/// Fichiers rattachés manuellement à `entity_id` pour ce type de média.
pub fn list_media_links(conn: &Connection, entity_id: &str, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT file_path FROM media_links WHERE entity_id = ?1 AND kind = ?2")?;
    let rows = stmt.query_map(params![entity_id, kind], |r| r.get::<_, String>(0))?;
    rows.collect()
}

// --- Contenu trouvé dans content/ (§12bis.1) --------------------------------

/// Indexe une voiture/circuit vivant dans `content/` : ligne minimale
/// `is_stock=1` (lecture seule). Ne touche pas un mod déjà présent (un vrai mod
/// géré n'est jamais « stock »).
///
/// `unmanaged` distingue le mod installé hors Pit Box du contenu de base
/// (§12bis.1bis). C'est le **seul** champ réécrit sur une ligne déjà indexée :
/// c'est ce qui reclasse les bases d'avant cette distinction — où tout ce qui
/// traînait dans `content/` passait pour du Kunos — sans perdre ce que
/// l'utilisateur y a saisi (nom repris à la main, description, tags manuels,
/// favori). Le `WHERE is_stock` protège d'une reclassification accidentelle
/// d'un mod géré qui porterait le même id.
pub fn upsert_stock_mod(
    conn: &Connection,
    id: &str,
    kind: &str,
    brand: Option<&str>,
    name: Option<&str>,
    created_at: &str,
    unmanaged: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO mods (id_interne, kind, brand, display_name, is_stock, is_unmanaged, created_at)
           VALUES (?1, ?2, ?3, ?4, 1, ?6, ?5)
           ON CONFLICT(id_interne) DO UPDATE SET is_unmanaged = ?6 WHERE is_stock = 1"#,
        params![id, kind, brand, name, created_at, unmanaged as i64],
    )?;
    Ok(())
}

/// Efface les **versions** synthétiques du contenu de base, en gardant les
/// lignes `mods` (§12bis.1). C'est ce qui permet de réindexer sans détruire ce
/// que l'utilisateur a mis dans l'overlay — nom repris à la main, description,
/// tags manuels, favori, catégorie.
///
/// Les versions, elles, se refabriquent à chaque passage avec un nouvel UUID :
/// sans cet effacement elles s'accumuleraient à chaque réindexation.
pub fn clear_stock_versions(conn: &Connection) -> rusqlite::Result<usize> {
    // `active_version_id` d'abord : la contrainte ne l'impose pas, mais laisser
    // un mod pointer une version qui vient d'être supprimée le rendrait
    // brièvement incohérent si l'indexation s'interrompait ici.
    conn.execute("UPDATE mods SET active_version_id = NULL WHERE is_stock = 1", [])?;
    conn.execute(
        "DELETE FROM versions WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )
}

/// Supprime les entrées de contenu de base **absentes de la liste** — celles
/// dont le dossier n'est plus dans `content/`. Complément de
/// `clear_stock_versions` : une réindexation qui ne détruit plus tout doit
/// quand même faire disparaître ce qui a été désinstallé du jeu.
pub fn delete_stock_absent(conn: &Connection, present: &[String]) -> rusqlite::Result<usize> {
    // Liste vide = plus rien sur disque : `NOT IN ()` étant invalide en SQL,
    // le cas se traite à part plutôt que de construire une requête bancale.
    if present.is_empty() {
        return delete_all_stock(conn);
    }
    let placeholders = std::iter::repeat_n("?", present.len()).collect::<Vec<_>>().join(",");
    let params = rusqlite::params_from_iter(present.iter());
    conn.execute(
        &format!("DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1 AND id_interne NOT IN ({placeholders}))"),
        rusqlite::params_from_iter(present.iter()),
    )?;
    conn.execute(
        &format!("DELETE FROM mods WHERE is_stock = 1 AND id_interne NOT IN ({placeholders})"),
        params,
    )
}

fn delete_all_stock(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )?;
    conn.execute("DELETE FROM mods WHERE is_stock = 1", [])
}

/// Supprime toutes les entrées de contenu de base (ré-indexation depuis zéro).
/// Les versions associées tombent par CASCADE. Les vrais mods ne sont pas touchés.
///
/// **Détruit aussi ce que l'utilisateur a saisi** sur ce contenu (nom,
/// description, tags manuels, favori) : réservé à la réinitialisation
/// explicitement demandée, jamais au réindex ordinaire (§9.3bis).
pub fn clear_stock(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )?;
    let n = conn.execute("DELETE FROM mods WHERE is_stock = 1", [])?;
    Ok(n)
}

/// Nombre d'entrées de contenu de base déjà indexées — sert à déclencher un
/// scan automatique au premier démarrage (§12bis.1).
pub fn count_stock(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM mods WHERE is_stock = 1", [], |r| r.get(0))
}

/// Ids indexés depuis `content/`, avec leur type — de quoi rejuger leur
/// classement sans relire le disque ([`crate::stock::reclassify_indexed_content`]).
pub fn list_stock_ids(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT id_interne, kind FROM mods WHERE is_stock = 1")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Repositionne le seul drapeau `is_unmanaged` d'une entrée déjà indexée
/// (§12bis.1bis). N'écrit rien d'autre : c'est ce qui permet de reclasser une
/// base existante sans toucher aux saisies de l'utilisateur.
pub fn set_unmanaged(conn: &Connection, id: &str, unmanaged: bool) -> rusqlite::Result<usize> {
    conn.execute(
        // `is_unmanaged != ?2` : le nombre de lignes touchées devient le nombre de
        // reclassements réels, pas le nombre d'entrées examinées.
        "UPDATE mods SET is_unmanaged = ?2 WHERE id_interne = ?1 AND is_stock = 1 AND is_unmanaged != ?2",
        params![id, unmanaged as i64],
    )
}

// --- Couches / extensions (§4.4) --------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRow {
    pub id: String,
    pub parent_id: String,
    pub parent_kind: String,
    pub name: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub added_count: i64,
    pub overwritten_count: i64,
    pub is_active: bool,
    pub priority: i64,
    pub imported_at: String,
}

/// Priorité à attribuer à une nouvelle couche du parent (max + 1 : empilée en tête).
pub fn next_layer_priority(conn: &Connection, parent_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(priority), -1) + 1 FROM layers WHERE parent_id = ?1",
        [parent_id],
        |r| r.get(0),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn insert_layer(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    parent_kind: &str,
    name: &str,
    library_path: &str,
    source_archive: Option<&str>,
    added_count: i64,
    overwritten_count: i64,
    priority: i64,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO layers
           (id, parent_id, parent_kind, name, library_path, source_archive,
            added_count, overwritten_count, priority, imported_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"#,
        params![
            id,
            parent_id,
            parent_kind,
            name,
            library_path,
            source_archive,
            added_count,
            overwritten_count,
            priority,
            imported_at
        ],
    )?;
    Ok(())
}

fn map_layer(row: &rusqlite::Row) -> rusqlite::Result<LayerRow> {
    Ok(LayerRow {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        parent_kind: row.get(2)?,
        name: row.get(3)?,
        library_path: row.get(4)?,
        source_archive: row.get(5)?,
        added_count: row.get(6)?,
        overwritten_count: row.get(7)?,
        is_active: row.get::<_, i64>(8)? != 0,
        priority: row.get(9)?,
        imported_at: row.get(10)?,
    })
}

const LAYER_SELECT: &str = "SELECT id, parent_id, parent_kind, name, library_path, source_archive, added_count, overwritten_count, is_active, priority, imported_at FROM layers";

/// Couches/extensions rattachées à une base (fiche détail, §4.4), par priorité.
pub fn list_layers(conn: &Connection, parent_id: &str) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!("{LAYER_SELECT} WHERE parent_id = ?1 ORDER BY priority"))?;
    let rows = stmt.query_map([parent_id], map_layer)?;
    rows.collect()
}

/// Toutes les couches d'un type (Car|Track), pour la vue transversale add-ons.
pub fn list_layers_by_kind(conn: &Connection, kind: &str) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!(
        "{LAYER_SELECT} WHERE parent_kind = ?1 ORDER BY parent_id, priority"
    ))?;
    let rows = stmt.query_map([kind], map_layer)?;
    rows.collect()
}

/// Couches **actives** d'une base, dans l'ordre de priorité (la + haute en dernier
/// → gagne à la superposition). Base de la composition (§4.4).
pub fn active_layers(conn: &Connection, parent_id: &str) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!(
        "{LAYER_SELECT} WHERE parent_id = ?1 AND is_active = 1 ORDER BY priority"
    ))?;
    let rows = stmt.query_map([parent_id], map_layer)?;
    rows.collect()
}

pub fn get_layer(conn: &Connection, id: &str) -> rusqlite::Result<Option<LayerRow>> {
    let mut stmt = conn.prepare(&format!("{LAYER_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_layer)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn set_layer_active(conn: &Connection, id: &str, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE layers SET is_active = ?2 WHERE id = ?1",
        params![id, active as i64],
    )?;
    Ok(())
}

pub fn set_layer_priority(conn: &Connection, id: &str, priority: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE layers SET priority = ?2 WHERE id = ?1", params![id, priority])?;
    Ok(())
}

pub fn delete_layer(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM layers WHERE id = ?1", [id])?;
    Ok(())
}

// --- Sous-éléments rattachés (§12bis.2) -------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubModRow {
    pub id: String,
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub is_active: bool,
    /// Faux si fourni avec le contenu initial du mod (découvert sur disque,
    /// §8) — non supprimable individuellement, seulement le mod entier.
    pub removable: bool,
    pub imported_at: String,
    /// Taille sur disque du dossier stocké, octets. Jamais mémorisée en base
    /// (le contenu d'un skin peut changer sous nos pieds) : mesurée à la
    /// demande, donc `None` partout sauf là où on la réclame explicitement
    /// (vue transversale, cf. `submods::list_by_type_sized`).
    pub size_bytes: Option<i64>,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_sub_mod(
    conn: &Connection,
    id: &str,
    sub_type: &str,
    parent_id: &str,
    name: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, source_archive, imported_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7)"#,
        params![id, sub_type, parent_id, name, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

/// Enregistre un skin de circuit découvert sur disque, fourni avec le
/// contenu initial du mod (§8) — jamais importé séparément par Pit Box,
/// donc `removable = 0` : reconnu et activable, mais non supprimable
/// individuellement (seulement le mod entier).
pub fn insert_bundled_track_skin(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    name: &str,
    library_path: &str,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, source_archive, removable, imported_at)
           VALUES (?1,'TRACK_SKIN',?2,?3,?4,NULL,0,?5)"#,
        params![id, parent_id, name, library_path, imported_at],
    )?;
    Ok(())
}

fn map_sub(row: &rusqlite::Row) -> rusqlite::Result<SubModRow> {
    Ok(SubModRow {
        id: row.get(0)?,
        sub_type: row.get(1)?,
        parent_id: row.get(2)?,
        name: row.get(3)?,
        library_path: row.get(4)?,
        source_archive: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        removable: row.get::<_, i64>(7)? != 0,
        imported_at: row.get(8)?,
        size_bytes: None,
    })
}

const SUB_SELECT: &str =
    "SELECT id, sub_type, parent_id, name, library_path, source_archive, is_active, removable, imported_at FROM sub_mods";

/// Sous-éléments rattachés à une entité (fiche détail, §12bis.3).
/// Sous-éléments (skins, sons) dont le parent n'existe plus (§9.3). Conservés
/// **délibérément** à la suppression du mod — voir `delete_mod` — mais devenus
/// inutiles dès qu'on ne compte plus réimporter le parent. Listés ici pour être
/// nettoyés sur décision de l'utilisateur, jamais automatiquement.
pub fn orphan_subs(conn: &Connection) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM sub_mods
          WHERE parent_id NOT IN (SELECT id_interne FROM mods)
          ORDER BY parent_id, name",
    )?;
    let rows = stmt.query_map([], map_sub)?;
    rows.collect()
}

pub fn list_subs_for_parent(conn: &Connection, parent_id: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE parent_id = ?1 ORDER BY name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([parent_id], map_sub)?;
    rows.collect()
}

/// Tous les sous-éléments d'un type (vue transversale, §12bis.3).
pub fn list_subs_by_type(conn: &Connection, sub_type: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE sub_type = ?1 ORDER BY parent_id, name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([sub_type], map_sub)?;
    rows.collect()
}

pub fn get_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_sub)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Existe-t-il déjà un sous-élément de ce type/parent/nom ? (idempotence import).
pub fn sub_exists(conn: &Connection, sub_type: &str, parent_id: &str, name: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sub_mods WHERE sub_type = ?1 AND parent_id = ?2 AND name = ?3",
        params![sub_type, parent_id, name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn delete_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sub_mods WHERE id = ?1", [id])?;
    Ok(())
}

/// Bascule exclusive du son actif d'une voiture (§12bis.2) : un seul SOUND actif
/// par parent. `id = None` désactive tout (retour au son d'origine).
pub fn set_active_sound(conn: &Connection, parent_id: &str, id: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET is_active = 0 WHERE parent_id = ?1 AND sub_type = 'SOUND'",
        [parent_id],
    )?;
    if let Some(id) = id {
        conn.execute("UPDATE sub_mods SET is_active = 1 WHERE id = ?1", [id])?;
    }
    Ok(())
}

/// Active/désactive un skin de circuit par nom (§8) — PAS exclusif,
/// contrairement au son : plusieurs TRACK_SKIN peuvent être `is_active` en
/// même temps pour un même circuit.
pub fn set_track_skin_active(conn: &Connection, parent_id: &str, name: &str, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET is_active = ?3 WHERE parent_id = ?1 AND sub_type = 'TRACK_SKIN' AND name = ?2",
        params![parent_id, name, active as i64],
    )?;
    Ok(())
}

// --- Apps (§12bis.4) --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
}

pub fn insert_app(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO apps (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)
           ON CONFLICT(id) DO UPDATE SET library_path = excluded.library_path"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

pub fn list_apps(conn: &Connection) -> rusqlite::Result<Vec<AppRow>> {
    let mut stmt =
        conn.prepare("SELECT id, library_path, source_archive, imported_at FROM apps ORDER BY id COLLATE NOCASE")?;
    let rows = stmt.query_map([], |r| {
        Ok(AppRow {
            id: r.get(0)?,
            library_path: r.get(1)?,
            source_archive: r.get(2)?,
            imported_at: r.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn get_app(conn: &Connection, id: &str) -> rusqlite::Result<Option<AppRow>> {
    let mut stmt = conn.prepare("SELECT id, library_path, source_archive, imported_at FROM apps WHERE id = ?1")?;
    let mut rows = stmt.query_map([id], |r| {
        Ok(AppRow {
            id: r.get(0)?,
            library_path: r.get(1)?,
            source_archive: r.get(2)?,
            imported_at: r.get(3)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

#[allow(dead_code)] // utilisé par les tests d'apps
pub fn app_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM apps WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn delete_app(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM apps WHERE id = ?1", [id])?;
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    Ok(())
}

// --- Mods « autres » (§7.3) -----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherModRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub is_priority: bool,
    pub is_active: bool,
    /// Chemins absolus des jonctions créées lors de la dernière activation.
    pub junctions: Vec<String>,
}

pub fn insert_other_mod(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO other_mods (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

fn map_other(row: &rusqlite::Row) -> rusqlite::Result<OtherModRow> {
    let junctions: String = row.get(6)?;
    Ok(OtherModRow {
        id: row.get(0)?,
        library_path: row.get(1)?,
        source_archive: row.get(2)?,
        imported_at: row.get(3)?,
        is_priority: row.get::<_, i64>(4)? != 0,
        is_active: row.get::<_, i64>(5)? != 0,
        junctions: json_arr(&junctions),
    })
}

const OTHER_SELECT: &str =
    "SELECT id, library_path, source_archive, imported_at, is_priority, is_active, junctions FROM other_mods";

pub fn list_other_mods(conn: &Connection) -> rusqlite::Result<Vec<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} ORDER BY id COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_other)?;
    rows.collect()
}

pub fn get_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_other)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn other_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM other_mods WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn set_other_priority(conn: &Connection, id: &str, priority: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_priority = ?2 WHERE id = ?1",
        params![id, priority as i64],
    )?;
    Ok(())
}

/// Bascule active/inactive + mémorise les jonctions créées (pour une
/// désactivation exacte plus tard).
pub fn set_other_active(conn: &Connection, id: &str, active: bool, junctions: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_active = ?2, junctions = ?3 WHERE id = ?1",
        params![
            id,
            active as i64,
            serde_json::to_string(junctions).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    Ok(())
}

pub fn delete_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM other_mods WHERE id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    Ok(())
}

// --- Poses explicitement autorisees (§4.6ter) -------------------------------

/// Enregistre qu'un mod a l'autorisation de poser ce chemin quoi qu'il occupe
/// deja. Idempotent.
pub fn mark_forced_extra(conn: &Connection, mod_id: &str, ac_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO forced_extras (mod_id, ac_path) VALUES (?1, ?2)",
        params![mod_id, ac_path],
    )?;
    Ok(())
}

pub fn is_forced_extra(conn: &Connection, mod_id: &str, ac_path: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM forced_extras WHERE mod_id = ?1 AND ac_path = ?2",
        params![mod_id, ac_path],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

/// Retire les autorisations d'un mod — a la suppression du mod, jamais a sa
/// desactivation : desactiver puis reactiver ne doit pas reposer la question.
pub fn clear_forced_extras(conn: &Connection, mod_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM forced_extras WHERE mod_id = ?1", [mod_id])?;
    Ok(())
}

// --- Dossiers proposes (§4.6ter) --------------------------------------------

#[derive(Debug, Clone)]
pub struct PendingFolderRow {
    pub id: String,
    pub archive: String,
    pub rel_path: String,
    pub library_path: String,
    pub owner_id: Option<String>,
    pub owner_kind: Option<String>,
    pub shape: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub skin_target: Option<String>,
    pub replaced: usize,
    pub found_at: String,
}

const PENDING_SELECT: &str = "SELECT id, archive, rel_path, library_path, owner_id, owner_kind, shape, title, \
     description, readme, skin_target, replaced, found_at FROM pending_folders";

fn map_pending(row: &rusqlite::Row) -> rusqlite::Result<PendingFolderRow> {
    Ok(PendingFolderRow {
        id: row.get(0)?,
        archive: row.get(1)?,
        rel_path: row.get(2)?,
        library_path: row.get(3)?,
        owner_id: row.get(4)?,
        owner_kind: row.get(5)?,
        shape: row.get(6)?,
        title: row.get(7)?,
        description: row.get(8)?,
        readme: row.get(9)?,
        skin_target: row.get(10)?,
        replaced: row.get::<_, i64>(11)?.max(0) as usize,
        found_at: row.get(12)?,
    })
}

/// `INSERT OR REPLACE` : reimporter la meme archive represente les memes
/// dossiers, et c'est voulu — la source est fraiche, la question se repose.
#[allow(clippy::too_many_arguments)]
pub fn insert_pending_folder(conn: &Connection, r: &PendingFolderRow) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT OR REPLACE INTO pending_folders
           (id, archive, rel_path, library_path, owner_id, owner_kind, shape, title,
            description, readme, skin_target, replaced, found_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        params![
            r.id,
            r.archive,
            r.rel_path,
            r.library_path,
            r.owner_id,
            r.owner_kind,
            r.shape,
            r.title,
            r.description,
            r.readme,
            r.skin_target,
            r.replaced as i64,
            r.found_at,
        ],
    )?;
    Ok(())
}

/// Les plus recents d'abord : ce qui vient d'etre importe est ce qu'on veut
/// trancher, et une vieille ligne encore en attente ne doit pas passer devant.
pub fn list_pending_folders(conn: &Connection) -> rusqlite::Result<Vec<PendingFolderRow>> {
    let mut stmt = conn.prepare(&format!(
        "{PENDING_SELECT} ORDER BY found_at DESC, rel_path COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], map_pending)?;
    rows.collect()
}

pub fn get_pending_folder(conn: &Connection, id: &str) -> rusqlite::Result<Option<PendingFolderRow>> {
    let mut stmt = conn.prepare(&format!("{PENDING_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_pending)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn delete_pending_folder(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM pending_folders WHERE id = ?1", [id])?;
    Ok(())
}

// --- Suivi d'usage (§6.5) ---------------------------------------------------

/// Pose/incrémente le marqueur « essayé » d'un mod au lancement d'une session.
pub fn mark_launched(conn: &Connection, mod_id: &str, ts: &str) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO usage (mod_id, launched, launch_count, last_launched)
           VALUES (?1, 1, 1, ?2)
           ON CONFLICT(mod_id) DO UPDATE SET
               launched = 1,
               launch_count = usage.launch_count + 1,
               last_launched = ?2"#,
        params![mod_id, ts],
    )?;
    Ok(())
}

/// Ids des mods déjà lancés au moins une fois par l'app.
pub fn launched_ids(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT mod_id FROM usage WHERE launched = 1")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}
