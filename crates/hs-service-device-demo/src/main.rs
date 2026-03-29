mod app;
mod bootstrap;
mod command_payload;
mod time;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
