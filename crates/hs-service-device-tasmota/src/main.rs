mod app;
mod bootstrap;
mod command_payload;
mod config;
mod tasmota_client;
mod time;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
