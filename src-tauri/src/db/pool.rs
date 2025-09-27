use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

pub fn init_pool(db_path: &Path) -> DbPool {
    let manager = SqliteConnectionManager::file(db_path);
    Pool::new(manager).expect("failed to create sqlite pool")
}
