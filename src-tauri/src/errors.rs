//! User-facing error identifiers.
//!
//! Backend errors travel to the UI as plain strings. Anything a user is meant
//! to read is therefore an **i18n key**, resolved by the frontend (see
//! `src/lib/errors.ts`) — never a sentence in one language, which could not be
//! translated. Same convention as `config::validate`.
//!
//! Technical details (IO, SQLite) keep their raw message: they are diagnostics,
//! not user guidance.

pub const MOD_NOT_FOUND: &str = "errors.modNotFound";
pub const MOD_UNKNOWN: &str = "errors.modUnknown";
pub const AC_NOT_CONFIGURED: &str = "errors.acNotConfigured";
pub const LIBRARY_NOT_CONFIGURED: &str = "errors.libraryNotConfigured";
pub const CM_NOT_CONFIGURED: &str = "errors.cmNotConfigured";
pub const SEVENZIP_NOT_CONFIGURED: &str = "errors.sevenzipNotConfigured";
pub const VERSION_NOT_FOUND: &str = "errors.versionNotFound";
pub const ACTIVE_VERSION_NOT_FOUND: &str = "errors.activeVersionNotFound";
pub const NO_ACTIVE_VERSION: &str = "errors.noActiveVersion";
pub const NO_VERSION_TO_ACTIVATE: &str = "errors.noVersionToActivate";
pub const NO_ACTIVE_VERSION_TO_EXPORT: &str = "errors.noActiveVersionToExport";
pub const VERSION_FILES_MISSING: &str = "errors.versionFilesMissing";
pub const LAYER_NOT_FOUND: &str = "errors.layerNotFound";
pub const APP_NOT_FOUND: &str = "errors.appNotFound";
pub const SUB_MOD_NOT_FOUND: &str = "errors.subModNotFound";
pub const SOUND_NOT_FOUND: &str = "errors.soundNotFound";
pub const TARGET_CAR_UNKNOWN: &str = "errors.targetCarUnknown";
pub const NOT_A_SOUND_MOD: &str = "errors.notASoundMod";
pub const BUNDLED_NOT_REMOVABLE: &str = "errors.bundledNotRemovable";
pub const STOCK_NOT_ACTIVATABLE: &str = "errors.stockNotActivatable";
pub const INCONSISTENT_STOCK: &str = "errors.inconsistentStock";
pub const EMPTY_STOCK_BACKUP: &str = "errors.emptyStockBackup";
pub const REAL_FOLDER_IN_CONTENT: &str = "errors.realFolderInContent";
pub const REAL_APP_FOLDER_UNTOUCHED: &str = "errors.realAppFolderUntouched";
pub const REAL_APP_FOLDER_EXISTS: &str = "errors.realAppFolderExists";
pub const NOT_A_JUNCTION: &str = "errors.notAJunction";
pub const NOT_DEPLOYED_BY_PITBOX: &str = "errors.notDeployedByPitbox";
// Chemin non-Windows (create_junction) : jamais atteint sur la cible réelle.
#[allow(dead_code)]
pub const JUNCTIONS_WINDOWS_ONLY: &str = "errors.junctionsWindowsOnly";
pub const KEPT_ARCHIVE_MISSING: &str = "errors.keptArchiveMissing";
pub const NO_KEPT_ARCHIVE: &str = "errors.noKeptArchive";
pub const NO_CONTENT_IN_ARCHIVE: &str = "errors.noContentInArchive";
pub const SHOWROOM_EXE_MISSING: &str = "errors.showroomExeMissing";
pub const DOCUMENTS_NOT_FOUND: &str = "errors.documentsNotFound";
pub const UNNAMED_MOD_FOLDER: &str = "errors.unnamedModFolder";
pub const PATH_OUTSIDE_RESOURCES: &str = "errors.pathOutsideResources";
pub const NOT_A_DIRECTORY: &str = "errors.notADirectory";
pub const REAL_FOLDER_GUARD: &str = "errors.realFolderGuard";
pub const MUSIC_FOLDER_EMPTY: &str = "errors.musicFolderEmpty";
pub const MEDIA_NOT_A_FILE: &str = "errors.mediaNotAFile";
