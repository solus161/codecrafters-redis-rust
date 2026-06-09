use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::rc::Rc;
use sha2::{ Sha256, Digest };

use crate::exceptions::CustomError;

#[derive(Hash, Eq, PartialEq)]
pub enum AuthFlags {
    NoPass,
}

impl Display for AuthFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPass => write!(f, "nopass")
        }
    }
}

pub struct Credential {
    username: Rc<str>,
    passwords: HashSet<String>,
    flags: HashSet<AuthFlags>,
}

impl Credential {
    pub fn new() -> Self {
        let mut defaut_flags = HashSet::new();
        defaut_flags.insert(AuthFlags::NoPass);

        Self {
            username: Rc::from("default"),
            passwords: HashSet::new(),
            flags: defaut_flags,
        }
    }
    
    pub fn username(&self) -> &str {
        self.username.as_ref()
    }
    
    pub fn flags(&self) -> Vec<&AuthFlags> {
        self.flags.iter().map(|p| p).collect()
    }

    pub fn passwords(&self) -> Vec<&str> {
        self.passwords.iter().map(|p| p.as_str()).collect()
    }
    
    pub fn set_password(&mut self, password: &str) {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let encoded = hex::encode(hasher.finalize());
        self.passwords.insert(encoded);

        // Remove nopass flag
        self.flags.remove(&AuthFlags::NoPass);
    }

    pub fn check_password(&self, password: &str) -> bool {
        self.passwords.contains(password)
    }

    pub fn add_flag(&mut self, flag: AuthFlags) -> bool {
        self.flags.insert(flag)
    }
}

pub struct Auth {
    credentials: HashMap<Rc<str>, Credential>,  // username-credential
    clients: HashMap<u64, Rc<str>>,             // client_id-username
}

impl Auth {
    pub fn new() -> Self {
        // Always havinga default user
        // Must be init once
        let default_user = Credential::new();
        let username = default_user.username.clone();
        let mut credentials = HashMap::new();
        credentials.insert(username, default_user);

        Self {
            credentials: credentials,
            clients: HashMap::new(),
        }
    }

    pub fn get() -> Rc<RefCell<Self>> {
        AUTH.with(|a| a.clone())
    }

    pub fn authenticate(&mut self, client_id: &u64, username: Option<&str>, password: Option<&str>)
        -> Result<(), CustomError>
    {
        // Default is the default user
        let msg = "WRONGPASS invalid username-password pair or user is disabled.";
        let err = CustomError::WrongUsernamePassword(msg.to_string());

        match (username, password) {
            (Some(u), Some(s)) =>{
                let username_rc = Rc::from(u);
                if let Some(credential) = self.credentials.get(&username_rc) {
                    if credential.check_password(s) {
                        let _ = self.clients.insert(*client_id, username_rc);
                        Ok(())
                    } else {
                        Err(err)
                    }
                } else {
                    Err(err)
                }
            },
            (Some(_), None) | (None, Some(_)) => {
                Err(err)
            },
            (None, None) => {
                // Assign default credential
                let username_rc = Rc::from("default");
                let _ = self.clients.insert(*client_id, username_rc);
                Ok(())
            }
        }
    }

    pub fn remove_auth(&mut self, client_id: &u64) {
        self.clients.remove(client_id);
    }

    pub fn get_username(&self, client_id: &u64) -> Result<&Rc<str>, CustomError> {
        self.clients.get(client_id).ok_or(
            CustomError::WrongUsernamePassword("Client not registered".to_string())
        )
    }

    pub fn get_credential(&self, username: &str) -> Option<&Credential> {
        self.credentials.get(&Rc::from(username))
    }

    pub fn get_credential_mut(&mut self, username: &str) -> Option<&mut Credential> {
        self.credentials.get_mut(&Rc::from(username))
    }
}

thread_local! {
    static AUTH: Rc<RefCell<Auth>> = Rc::new(RefCell::new(Auth::new()))
}
