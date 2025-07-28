    //! Node implementations

pub mod memory;
pub mod database;
pub mod filesystem;

pub use memory::MemoryNode;
pub use database::DatabaseNode;
pub use filesystem::FilesystemNode; 