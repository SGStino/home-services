mod app;
mod config;
mod mapper;
mod matter_ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
