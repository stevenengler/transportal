use rand::Rng;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct Session<T> {
    data: T,
    expires: SystemTime,
}

impl<T> Session<T> {
    pub fn new(data: T) -> Self {
        // approximately 4 months
        const EXPIRES: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 4);
        let expires = SystemTime::now().checked_add(EXPIRES).unwrap();

        Self { data, expires }
    }

    #[inline]
    pub fn data(&self) -> &T {
        // We don't check the expiration time here and we'll let the current (possibly expired)
        // session continue to avoid breaking any existing flows. We already allow the caller to
        // hold the `&T` past the expiration time anyways, since there's not really any way to
        // prevent this.
        &self.data
    }

    pub fn expired(&self) -> bool {
        SystemTime::now() >= self.expires
    }

    pub fn expires(&self) -> SystemTime {
        self.expires
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct SessionSecret(u128);

impl SessionSecret {
    pub fn new(val: u128) -> Self {
        Self(val)
    }

    /// Returns an object implementing `Display` that will write the cookie value and attributes,
    /// excluding the cookie name (the `cookie_name=` component).
    pub fn as_cookie(&self, secure_attr: bool, expire: Option<Duration>) -> SessionCookieDisplay {
        SessionCookieDisplay {
            secret: self.0,
            secure_attr,
            expire,
        }
    }
}

impl std::fmt::Debug for SessionSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<secret>")
    }
}

#[derive(Copy, Clone)]
pub struct SessionCookieDisplay {
    secret: u128,
    secure_attr: bool,
    expire: Option<Duration>,
}

impl std::fmt::Display for SessionCookieDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Self {
            secret,
            secure_attr,
            expire,
        } = self;

        const U128_MAX_DIGITS: usize = match u128::MAX.checked_ilog10() {
            Some(x) => x as usize + 1,
            None => unreachable!(),
        };

        // pad the secret to a constant length
        write!(f, "{secret:0U128_MAX_DIGITS$}; HttpOnly; SameSite=Lax;")?;

        if *secure_attr {
            write!(f, " Secure;")?;
        }

        if let Some(expire) = expire {
            write!(f, " Max-Age={};", expire.as_secs())?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SessionManager<T> {
    sessions: RwLock<HashMap<SessionSecret, Arc<Session<T>>>>,
}

impl<T> SessionManager<T> {
    pub fn session(&self, secret: SessionSecret) -> Option<Arc<Session<T>>> {
        let session = self.sessions.read().unwrap().get(&secret).cloned();

        // if the session has expired, remove it and don't return a session
        if let Some(ref session) = session {
            if session.expired() {
                self.remove_session(secret)?;
                return None;
            }
        }

        session
    }

    pub fn new_session(&self, session: Session<T>) -> SessionSecret {
        let mut rng = rand::thread_rng();

        let mut sessions = self.sessions.write().unwrap();

        // the chance of collisions with a 128 bit random number is very low
        let session_secret = loop {
            let x = SessionSecret::new(rng.gen());
            if !sessions.contains_key(&x) {
                break x;
            }
        };

        sessions.insert(session_secret, Arc::new(session));

        session_secret
    }

    pub fn remove_session(&self, secret: SessionSecret) -> Option<Arc<Session<T>>> {
        self.sessions.write().unwrap().remove(&secret)
    }
}

impl<T> Default for SessionManager<T> {
    fn default() -> Self {
        Self {
            sessions: Default::default(),
        }
    }
}
