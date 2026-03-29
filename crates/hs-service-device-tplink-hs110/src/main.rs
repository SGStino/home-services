mod app;
mod bootstrap;
mod command_payload;
mod config;
mod hs110_client;
mod time;
mod tplink_protocol;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
