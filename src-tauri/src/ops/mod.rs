pub mod archive;
pub mod delete;
pub mod error;
pub mod space;
pub mod undo;

pub use archive::{ArchiveConfig, ArchiveManager, ArchiveProgress, ArchiveResult};
pub use delete::{DeleteCandidate, DeleteConfig, DeleteManager, DeleteResult};
pub use error::{ErrorContext, ErrorMessage, OpsError, OpsResult};
pub use space::{SpaceCheck, SpaceInfo, SpaceManager};
pub use undo::{BatchInfo, UndoManager, UndoResult};

// Re-export commonly used types
pub use crate::models::{ActionType, NewAction};
