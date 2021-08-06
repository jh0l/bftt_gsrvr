use serde::Deserialize;

#[derive(Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub password: String,
}
