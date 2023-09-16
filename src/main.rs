use clap::Parser;
use log::{error, info};
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use simwatch_grpc::{
  config::read_config,
  manager::Manager,
  service::{camden::camden_server::CamdenServer, CamdenService},
};
use std::sync::Arc;
use tonic::transport::Server;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
struct Args {
  #[arg(short, default_value = "/etc/simwatch/simwatch-grpc.toml")]
  config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  let config = read_config(&args.config);
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
