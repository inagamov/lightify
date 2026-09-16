use std::sync::{Arc, RwLock};

use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig};

#[derive(Clone)]
pub struct SessionHandle {
    session: Arc<RwLock<Session>>,
    config: SessionConfig,
    cache: Cache,
}

impl SessionHandle {
    pub fn new(config: SessionConfig, cache: Cache) -> Self {
        let session = Session::new(config.clone(), Some(cache.clone()));
        Self {
            session: Arc::new(RwLock::new(session)),
            config,
            cache,
        }
    }

    pub fn get(&self) -> Session {
        self.session.read().expect("session lock poisoned").clone()
    }

    pub fn reconnect(&self) -> Session {
        let mut current = self.session.write().expect("session lock poisoned");
        if !current.is_invalid() {
            current.shutdown();
        }
        *current = Session::new(self.config.clone(), Some(self.cache.clone()));
        current.clone()
    }

    pub fn credentials(&self) -> Option<Credentials> {
        self.cache.credentials()
    }

    pub fn shutdown(&self) {
        let session = self.session.read().expect("session lock poisoned");
        if !session.is_invalid() {
            session.shutdown();
        }
    }
}
