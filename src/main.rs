use std::sync::Arc;

use grpcamden::{
  config::read_config,
  manager::Manager,
  service::{camden::camden_server::CamdenServer, CamdenService},
};
use log::{error, info};
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use tonic::transport::Server;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // TODO cmdline flag -c
  let config = read_config(None);
  let addr = config.grpc.listen.parse().unwrap();

  TermLogger::init(
    config.log.level,
    Config::default(),
    TerminalMode::Stdout,
    ColorChoice::Always,
  )
  .unwrap();

  info!("starting camden server version {}", VERSION);
  let m = Manager::new(config.clone()).await;
  let m = Arc::new(m);

  {
    let m = m.clone();
    tokio::spawn(async move {
      let res = m.run().await;
      if let Err(err) = res {
        error!("error running manager: {err:?}");
      }
    });
  }

  let svc = CamdenService::new(m);
  let svc = CamdenServer::new(svc);
  Server::builder().add_service(svc).serve(addr).await?;
  Ok(())
}
