
// TODO: Deserialize from: https://crates.io/crates/serde_kdl

#[derive(Debug, Clone)]
pub struct SimpleAuthConfig {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub simple_auth: Option<SimpleAuthConfig>,
    // TODO: In the future, the SASL server config will go here.
}
