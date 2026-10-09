//! The schema: `init` creates the tables of a new base, `migrate` brings an
//! older one up to date with idempotent `ALTER`s (§3).

use super::*;

/// Ajoute les colonnes L2 aux bases déjà créées en L1 (ALTER idempotent).
pub(super) fn migrate(conn: &Connection) -> rusqlite::Result<()> {
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
        // Mod installé hors Pit Box, trouvé dans content/ à l'indexation (§8.2).
        "is_unmanaged INTEGER NOT NULL DEFAULT 0",
        // Where the cached spec columns came from, field → source (FICHE§6.3).
        "tech_marks TEXT NOT NULL DEFAULT '{}'",
    ];
    for col in cols {
        // Ignore l'erreur « duplicate column » si la colonne existe déjà.
        let _ = conn.execute(&format!("ALTER TABLE mods ADD COLUMN {col}"), []);
    }
    // Date de publication estimée depuis les dates de fichiers (§6.2).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN published_at TEXT", []);
    // Taille sur disque de la version, octets (§10).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN size_bytes INTEGER", []);
    // Archive/dossier source conservé (§10/§11), si le réglage était activé à
    // l'import de cette version. Rend possible « Réinstaller depuis l'archive
    // source ». `NULL` = non conservé (comportement par défaut).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN kept_archive_path TEXT", []);
    // Showcase (ESPACE§4.1): whether the version still has its files. Every
    // version written before is complete, hence the default; `freed_*` say
    // when it lost them and what that gave back. `source_*` is where its
    // archive came from (ESPACE§8.2), when Windows kept it.
    let _ = conn.execute(
        "ALTER TABLE versions ADD COLUMN content_state TEXT NOT NULL DEFAULT 'full'",
        [],
    );
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN freed_at TEXT", []);
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN freed_bytes INTEGER", []);
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN source_site TEXT", []);
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN source_file_name TEXT", []);
    // A layer or an attached skin/sound follows its mod into the showcase
    // (ESPACE§5.4): its row stays, its files go. An app or an "other" mod has
    // the state without the gesture (ESPACE§5.6): only an imported library
    // export brings one in without its files.
    for table in ["layers", "sub_mods", "apps", "other_mods"] {
        let _ = conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN content_state TEXT NOT NULL DEFAULT 'full'"),
            [],
        );
        let _ = conn.execute(&format!("ALTER TABLE {table} ADD COLUMN freed_at TEXT"), []);
    }
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
    // Auteur d'un sous-élément : saisi à la main, parce qu'il n'est écrit nulle
    // part dans les fichiers. Un `.bank` FMOD ne le porte pas, et le lire dans
    // une notice serait une devinette sur du texte libre.
    let _ = conn.execute("ALTER TABLE sub_mods ADD COLUMN author TEXT", []);

    // --- Saisie utilisateur, toutes entités (REFONTE§6.1 et REFONTE§9.4) ---
    //
    // La note et le nom d'affichage cessent d'être un privilège des mods :
    // une couche s'appelle `spa2022-release_V1-03.rar` et c'est exactement
    // l'objet qui a besoin d'être renommé. Même principe que les colonnes
    // `*_user` de `mods` : la saisie vit **à côté** du champ dérivé du
    // fichier, jamais à sa place — sinon la première mise à jour du mod
    // l'écrase. Les erreurs « duplicate column » sont ignorées comme plus
    // haut : c'est ce qui rend ces ALTER idempotents.
    for table in ["mods", "sub_mods", "apps", "other_mods", "layers"] {
        let _ = conn.execute(&format!("ALTER TABLE {table} ADD COLUMN notes_user TEXT"), []);
    }
    // `mods` a déjà le sien depuis L2 : seule la note lui manquait.
    for table in ["sub_mods", "apps", "other_mods", "layers"] {
        let _ = conn.execute(&format!("ALTER TABLE {table} ADD COLUMN display_name_user TEXT"), []);
    }
    // Rattachement corrigé à la main (REFONTE§2.3). Seule la **correction**
    // est stockée : la déduction se recalcule à chaque lecture, `attach.rs`
    // dit pourquoi un rattachement périmé serait pire que pas de rattachement.
    let _ = conn.execute("ALTER TABLE other_mods ADD COLUMN attachment_user TEXT", []);

    // Article rendu, table des matières et crédits d'images de l'onglet
    // Wikipédia (§7.3). Ajoutés après coup : une base écrite par la version
    // précédente n'a que le texte brut, et c'est `wiki::store::CONTENT_VERSION`
    // qui la vide — un ALTER ne rétro-remplit rien.
    let _ = conn.execute("ALTER TABLE wiki_cache ADD COLUMN html TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute(
        "ALTER TABLE wiki_cache ADD COLUMN sections TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE wiki_cache ADD COLUMN images TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    Ok(())
}

pub(super) fn init(conn: &Connection) -> rusqlite::Result<()> {
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
            category          TEXT,                   -- tag # principal (§5)
            categories        TEXT NOT NULL DEFAULT '[]', -- catégories circuit multi-valué (§5)
            country           TEXT,
            is_favorite       INTEGER NOT NULL DEFAULT 0,
            tags_from_rule    TEXT NOT NULL DEFAULT '[]',
            tags_manual       TEXT NOT NULL DEFAULT '[]',
            drivetrain        TEXT,
            engine_pos        TEXT,
            aspiration        TEXT,
            engine_config     TEXT,
            gearbox           TEXT,
            tech_marks        TEXT NOT NULL DEFAULT '{}', -- source of the five above (FICHE§6.3)
            source_pack       TEXT,                   -- pack d'origine (§4.4)
            source_url        TEXT,                   -- URL d'origine (§4.4)
            is_stock          INTEGER NOT NULL DEFAULT 0, -- indexé depuis content/ (§8.1)
            is_unmanaged      INTEGER NOT NULL DEFAULT 0, -- ... et pas du Kunos (§8.2)
            active_version_id TEXT,
            created_at        TEXT NOT NULL
        );

        -- Sous-éléments rattachés à une voiture/circuit (§8.3) : skins, sons.
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

        -- Apps Python (§8.4) : type autonome, activable par junction.
        CREATE TABLE IF NOT EXISTS apps (
            id             TEXT PRIMARY KEY,       -- nom du dossier de l'app
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            imported_at    TEXT NOT NULL
        );

        -- Suivi d'usage propre à l'app (§6) : marqueur « déjà essayé » définitif
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
            size_bytes        INTEGER                 -- taille sur disque, octets (§10)
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

        -- Autres mods (§7.3) et Apps (§8.4) capturés par un profil : ni
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
        -- `mod_id` is nullable: a decision can be about a leftover no mod
        -- claims. No foreign key for that reason, and because a mod in the
        -- showcase (ESPACE R2) keeps its journal: only a **complete**
        -- deletion removes it, explicitly (`delete_mod`, ESPACE§5.3).
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

        -- Answers given to proposed folders (§4.6ter), per owner and folder
        -- name: a mod updated with the same `Wallpapers/` gets the same answer
        -- again instead of the same question. Keyed by the name the author gave
        -- the folder (lower-case), not by the archive path, whose wrapping
        -- folder changes with every version. Written by `pending::resolve`, and
        -- by removing what an answer produced (a kept folder, a layer), which
        -- becomes "do not import".
        CREATE TABLE IF NOT EXISTS pending_answers (
            owner_id    TEXT NOT NULL,
            owner_kind  TEXT NOT NULL,
            name        TEXT NOT NULL,
            action      TEXT NOT NULL,
            answered_at TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (owner_id, owner_kind, name)
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

        -- Enrichissement Wikipédia de la fiche (docs/SPEC-wikipedia-fiche-detail.md
        -- §3). Trois tables : l'appariement, que l'utilisateur peut corriger et
        -- qui circulera un jour entre installations (§10) ; le contenu rapporté
        -- du réseau ; et le cache négatif, sans lequel chaque ouverture d'une
        -- fiche sans correspondance relancerait une résolution complète.
        --
        -- `mod_key` est le nom du dossier du mod, donc `mods.id_interne` —
        -- **sans clé étrangère, délibérément** : les `foreign_keys` sont à ON
        -- dans cette base, et un appariement est précisément ce qui doit
        -- survivre à la suppression puis au réimport du mod.
        CREATE TABLE IF NOT EXISTS wiki_link (
            mod_key     TEXT PRIMARY KEY,
            entity_id   TEXT NOT NULL,          -- Q-id Wikidata, jamais une URL (WIKI§3.1)
            source      TEXT NOT NULL,          -- 'auto' | 'import' | 'manual', par précédence
            resolved_at TEXT NOT NULL
        );

        -- Clé composite (entité, langue **demandée**) : la langue réellement
        -- servie peut différer quand la chaîne de repli du WIKI§5.2 est descendue
        -- sur l'anglais, et elle se relit dans `article_url` — c'est pourquoi
        -- cette URL est stockée et jamais reconstruite (WIKI§3.2).
        CREATE TABLE IF NOT EXISTS wiki_cache (
            entity_id       TEXT NOT NULL,
            lang            TEXT NOT NULL,
            article_title   TEXT NOT NULL,
            article_url     TEXT NOT NULL,
            revision_id     INTEGER,
            extract         TEXT NOT NULL,
            parent_entity   TEXT,               -- non nul = repli sur l'entité parente (WIKI§5.3)
            available_langs TEXT NOT NULL DEFAULT '[]',
            fetched_at      TEXT NOT NULL,
            PRIMARY KEY (entity_id, lang)
        );

        -- The tech sheet (FICHE§6.1). What the sources say, recomputed and
        -- never edited: per version, since an update may change the physics
        -- ('' for the game's own content and for what the rules settle).
        -- Every source is kept, not only the winner, so the edit mode can say
        -- where a value comes from and "revert" knows where to go back to.
        CREATE TABLE IF NOT EXISTS tech_facts (
            mod_id     TEXT NOT NULL REFERENCES mods(id_interne) ON DELETE CASCADE,
            version_id TEXT NOT NULL DEFAULT '',
            field      TEXT NOT NULL,
            source     TEXT NOT NULL,          -- 'physics' | 'ui' | 'table' | 'rules'
            value      TEXT NOT NULL,          -- JSON
            PRIMARY KEY (mod_id, version_id, field, source)
        );

        -- What the user decided, never recomputed (FICHE R6). **No foreign
        -- key, deliberately**, like `wiki_link`: a decision survives a
        -- deletion followed by a reimport. A complete deletion removes it
        -- explicitly (`delete_mod`).
        CREATE TABLE IF NOT EXISTS tech_user (
            mod_id    TEXT NOT NULL,
            field     TEXT NOT NULL,
            value     TEXT,                    -- JSON; NULL = forced "unknown"
            edited_at TEXT NOT NULL,
            PRIMARY KEY (mod_id, field)
        );

        CREATE TABLE IF NOT EXISTS wiki_no_match (
            mod_key      TEXT PRIMARY KEY,
            attempted_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_versions_mod ON versions(mod_id);
        CREATE INDEX IF NOT EXISTS idx_history_mod  ON history(mod_id);
        CREATE INDEX IF NOT EXISTS idx_mods_idhash  ON mods(identity_hash);
        CREATE INDEX IF NOT EXISTS idx_pe_profile   ON profile_entries(profile_id);
        CREATE INDEX IF NOT EXISTS idx_pee_profile  ON profile_extra_entries(profile_id);
        "#,
    )
}
