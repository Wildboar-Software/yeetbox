use std::{collections::HashMap, net::IpAddr};
use chrono::prelude::*;
use crate::grpc::remotefs::{AuthenticateArg, AuthenticateResult};

pub type UserId = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Session {
    pub user_id: Option<UserId>,
    pub auth_mech: String,
    pub start_time: DateTime<Utc>,
    pub peer_ip: IpAddr,
    pub peer_port: u16,
    // TODO: Permissions?
}

#[tonic::async_trait]
pub trait Authenticator {
    async fn get_auth_mechs (&self) -> Vec<String>;
    async fn attempt_authn (&self, arg: &AuthenticateArg) -> anyhow::Result<AuthenticateResult>;
    async fn check_authn (&self, peer_ip: IpAddr, peer_port: u16) -> anyhow::Result<Option<Session>>;
}

impl AuthenticateResult {

    pub fn reject () -> Self {
        AuthenticateResult{
            decision: Some(false),
            all_auth_disabled: false,
            user_disabled: false,
        }
    }

    pub fn accept () -> Self {
        AuthenticateResult{
            decision: Some(true),
            all_auth_disabled: false,
            user_disabled: false,
        }
    }

}

pub struct SimpleAuth {
    pub authenticated_peers: HashMap<(IpAddr, u16), Session>,
    pub db: HashMap<String, String>, // Username:Password
}

impl SimpleAuth {

    pub fn new () -> Self {
        Self {
            authenticated_peers: HashMap::new(),
            db: HashMap::new(),
        }
    }

}

#[tonic::async_trait]
impl Authenticator for SimpleAuth {

    async fn get_auth_mechs (&self) -> Vec<String> {
        vec!["ANONYMOUS".to_string(), "PLAIN".to_string()]
    }

    async fn attempt_authn (&self, arg: &AuthenticateArg) -> anyhow::Result<AuthenticateResult> {
        if arg.mechanism == "ANONYMOUS" {
            return Ok(AuthenticateResult::accept());
        }
        if arg.mechanism != "PLAIN" {
            return Ok(AuthenticateResult::reject());
        }
        // Prevents DoS via gigantic auth strings.
        if arg.assertion.len() > 1000 {
            return Ok(AuthenticateResult::reject());
        }
        let mut splitter = arg.assertion.split(|c| *c == b'\0');
        let maybe_authzid = splitter.next();
        let maybe_authcid = splitter.next();
        let maybe_passwd = splitter.next();
        let maybe_whatever = splitter.next();

        // There MUST be two nulls EXACTLY.
        if
            maybe_authzid.is_none()
            || maybe_authcid.is_none()
            || maybe_passwd.is_none()
            || maybe_whatever.is_some()
        {
            return Ok(AuthenticateResult::reject());
        }
        let authzid = maybe_authzid.unwrap();
        let authcid = maybe_authcid.unwrap();
        let passwd = maybe_passwd.unwrap();

        // The simple authenticator has no concept of authorization identity.
        // So any assertion of authzid that differs from authcid is rejected.
        if authzid.len() > 0 && authzid != authcid {
            return Ok(AuthenticateResult::reject());
        }

        let authcid = std::str::from_utf8(authcid)?;
        let passwd = std::str::from_utf8(passwd)?;
        let success = self.db.get(authcid).is_some_and(|pw| pw.as_str() == passwd);

        if success {
            Ok(AuthenticateResult::accept())
        } else {
            Ok(AuthenticateResult::reject())
        }
    }

    async fn check_authn (&self, peer_ip: IpAddr, peer_port: u16) -> anyhow::Result<Option<Session>> {
        Ok(self.authenticated_peers.get(&(peer_ip, peer_port)).cloned())
    }

}

// pub fn check_auth(req: Request<()>) -> Result<Request<()>, Status> {
//     let token: MetadataValue<_> = "Bearer some-secret-token".parse().unwrap();

//     let r = req.into_inner();

//     match req.metadata().get("authorization") {
//         Some(t) if token == t => Ok(req),
//         _ => Err(Status::unauthenticated("No valid auth token")),
//     }
// }
