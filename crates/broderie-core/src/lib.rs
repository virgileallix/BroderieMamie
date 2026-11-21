//! Cœur du logiciel « Les broderies de Mamie » : lecture des fichiers de broderie,
//! aperçus, et bibliothèque sans doublons. Indépendant de l'interface graphique.

pub mod design;
pub mod library;
pub mod render;

pub use design::{file_hash, parse, Design, Format, ParseError};
pub use library::{Entry, EntryUpdate, ImportReport, Library};
