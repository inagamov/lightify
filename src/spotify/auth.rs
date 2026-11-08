use std::path::Path;

use anyhow::Context;
use librespot::core::cache::Cache;

pub fn open_cache(cache_dir: &Path) -> anyhow::Result<Cache> {
    Cache::new(Some(cache_dir.to_path_buf()), None, None, None)
        .context("could not open credential cache")
}
