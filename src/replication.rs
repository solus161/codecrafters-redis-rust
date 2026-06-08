use std::cell::RefCell;
use std::collections::hash_set::Iter;
use std::rc::Rc;

// Just a dump struct to store master and slave

use std::collections::HashSet;


pub struct ClientTable {
    // clients: HashMap<u64, TcpClient>,
    pub master: Option<u64>,
    pub slaves: Option<HashSet<u64>>,
}

impl ClientTable {
    pub fn new() -> Self {
        Self {master: None, slaves: None}
    }

    pub fn get() -> Rc<RefCell<Self>> {
        CLIENT_TABLE.with(|t| t.clone())
    }

    pub fn remove_client(&mut self, client_id: &u64) {
        if let Some(x) = &self.master {
            if *x == *client_id {self.master = None };
        };
        if let Some(h) = &mut self.slaves {
            h.remove(client_id);
        };
        // self.clients.remove(&client_id)
    }
    
    pub fn is_master(&self, client_id: &u64) -> bool {
        if let Some(x) = self.master {
            x == *client_id
        } else {
            false
        }
    }

    pub fn is_slave(&self, client_id: &u64) -> bool {
        if let Some(h) = &self.slaves {
            h.contains(client_id)
        } else {
            false
        }
    }
    
    pub fn set_master(&mut self, client_id: u64) -> Result<(), String> {
        self.master = Some(client_id);
        Ok(())
    }

    pub fn list_slave(&self) ->  Option<Iter<'_, u64>> {
        if let Some(l) = &self.slaves {
            Some(l.iter())
        } else {
            None
        }
    }

    pub fn set_slave(&mut self, client_id: u64) -> Result<(), String> {
        self.slaves.get_or_insert_default().insert(client_id);
        Ok(())
    }

    pub fn remove_slave(&mut self, client_id: &u64) {
        if let Some(l) = &mut self.slaves {
            l.remove(client_id);
        };
    }
}

thread_local! {
    static CLIENT_TABLE: Rc<RefCell<ClientTable>> = Rc::new(RefCell::new(ClientTable::new()));
}
