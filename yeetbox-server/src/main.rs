mod authn;
mod authz;
mod config;
mod grpc;
mod logging;
mod service;
mod storage;
mod time64;
mod utils;
mod web;
use authn::Authenticator;
use authz::Authorizer;
use config::Config;
use grpc::remotefs::file_system_service_server::{FileSystemService, FileSystemServiceServer};

use logging::get_default_log4rs_config;
use storage::database::DatabaseStorage;
use storage::Storage;
use tonic::{transport::Server, Request, Response, Status};
// use warp::Filter;
// use warp::http::StatusCode;
// use chrono::prelude::*;
// use log::{debug, error, trace, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
// use web::{LocationsPage, Props};
// use std::convert::Infallible;
// use std::rc::Rc;
// use utils::grpc_timestamp_to_chrono;

#[derive(Clone)]
pub struct FileSystemServiceProvider {
    pub authn: Arc<Mutex<dyn Authenticator + Send + Sync + 'static>>,
    pub authz: Arc<Mutex<dyn Authorizer + Send + Sync + 'static>>,
    pub storage: Arc<Mutex<dyn Storage + Send + Sync + 'static>>,
    pub config: Arc<Config>,
}

#[cfg(not(target_os = "wasi"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log4rs::init_config(get_default_log4rs_config()).unwrap();
    let addr = "127.0.0.1:50051".parse()?;
    let authenticator = Arc::new(Mutex::new(authn::SimpleAuth::new()));
    let authorizer = Arc::new(Mutex::new(authz::simple::SimpleAuthz::new()));
    let storage = Arc::new(Mutex::new(DatabaseStorage::new()));
    let fs_provider = FileSystemServiceProvider {
        authn: authenticator,
        authz: authorizer,
        storage,
        config: Arc::new(Config {
            simple_auth: None,
        }),
    };

    let fs_server = FileSystemServiceServer::new(fs_provider);
    // let fs_with_auth = FileSystemServiceServer::with_interceptor(fs_server, interceptor);

    log::info!("Listening on {}", addr);
    Server::builder()
        .add_service(fs_server)
        .serve(addr).await?;
    // tokio::spawn(
    //     Server::builder()
    //         .add_service(fs_server)
    //         .serve(addr),
    // );

    // let locations_path = warp::path!("locations" / String)
    //     .and(with_storage(storage))
    //     .and_then(|token, storage| {
    //         render_locations_path(token, storage)
    //     });

    // warp::serve(locations_path)
    //     .run(([127, 0, 0, 1], 3030))
    //     .await;

    Ok(())
}
