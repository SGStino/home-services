mod app;
mod config;
mod esphome;
mod time;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
