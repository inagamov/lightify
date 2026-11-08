use std::path::Path;

use librespot::core::cache::Cache;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("could not open credential cache: {0}")]
    Cache(#[source] librespot::core::Error),
}

pub fn open_cache(cache_dir: &Path) -> Result<Cache, AuthError> {
    Cache::new(Some(cache_dir.to_path_buf()), None, None, None).map_err(AuthError::Cache)
}
