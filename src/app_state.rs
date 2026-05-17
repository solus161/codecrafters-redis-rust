use std::cell::RefCell;
use std::rc::Rc;

pub struct AppState {
    role: Option<String>, 
}

impl AppState {
    fn init() -> Self {
        Self {role: Some("master".to_string())}
    }

    pub fn get() -> Rc<RefCell<Self>>  {
        STATE.with(|state| return state.clone())
    }
    
    pub fn get_replication(&self) -> String {
        format!("role:{}", self.role.clone().unwrap_or("master".to_string()))
    }

}

thread_local! {
    static STATE: Rc<RefCell<AppState>> = Rc::new(RefCell::new(AppState::init()));
}

