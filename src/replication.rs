use std::cell::RefCell;
use std::collections::hash_set::Iter;
use std::collections::HashMap;
use std::rc::Rc;

// Just a dump struct to store master and slave
use crate::TcpClient;
use crate::cmd_builder::Cmd;

use std::collections::HashSet;

pub struct ClientTable {
    // clients: HashMap<u64, TcpClient>,
    master: Option<u64>,
    pub slaves: Option<HashSet<u64>>,
}

impl ClientTable {
    pub fn new() -> Self {
        Self {master: None, slaves: None}
    }

    pub fn get() -> Rc<RefCell<Self>> {
        CLIENT_TABLE.with(|t| t.clone())
    }

    // pub fn get_mut_client(&mut self, client_id: &u64) -> Option<&mut TcpClient> {
    //     self.clients.get_mut(client_id)
    // }

    // pub fn add_client(&mut self, client: TcpClient, id: u64) {
    //     self.clients.insert(id, client);
    // }

    pub fn remove_client(&mut self, client_id: &u64) {
        if let Some(x) = &self.master {
            if *x == *client_id {self.master = None };
        };
        if let Some(h) = &mut self.slaves {
            if h.contains(client_id) { h.remove(client_id); };
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
        // if self.clients.contains_key(&client_id) {
        //     self.master = Some(client_id);
        //     Ok(())
        // } else {
        //     Err(format!("Client id {} does not exist", &client_id))
        // }
        
    }

    pub fn list_slave(&self) ->  Option<Iter<'_, u64>> {
        if let Some(l) = &self.slaves {
            Some(l.iter())
        } else {
            None
        }
    }

    pub fn set_slave(&mut self, client_id: u64) -> Result<(), String> {
        println!("Set client {} as slave", &client_id);
        self.slaves.get_or_insert_default().insert(client_id);
        Ok(())
        // if self.clients.contains_key(&client_id) {
        //     if let Some(l) = &mut self.slaves {
        //         l.insert(client_id);
        //     };
        //     Ok(())
        // } else {
        //     Err(format!("Client id {} does not exist", &client_id))
        // }
    }

    pub fn remove_slave(&mut self, client_id: &u64) {
        if let Some(l) = &mut self.slaves {
            l.remove(client_id);
        };
    }

    // pub fn broadcaste(&mut self, buf: Vec<u8>) {
    //     if let Some(h) = &self.slaves {
    //         let mut vec_response: Vec<(u64, Vec<u8>)> = Vec::new();
    //         h.iter().for_each(|id| {
    //             vec_response.push((*id, buf));
    //         })
    //     };
    // }
}

thread_local! {
    static CLIENT_TABLE: Rc<RefCell<ClientTable>> = Rc::new(RefCell::new(ClientTable::new()))
}
