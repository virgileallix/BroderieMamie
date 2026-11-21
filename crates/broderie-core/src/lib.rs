//! Cœur du logiciel « Les broderies de Mamie » : lecture des fichiers de broderie,
//! aperçus, et bibliothèque sans doublons. Indépendant de l'interface graphique.

mod compress;
pub mod design;
mod formats;
mod palettes;
pub mod library;
pub mod render;

pub use design::{file_hash, parse, parse_named, shape_similarity, Design, Format, ParseError};
pub use library::{DuplicateGroup, Entry, EntryUpdate, GroupChoice, ImportReport, Library};
