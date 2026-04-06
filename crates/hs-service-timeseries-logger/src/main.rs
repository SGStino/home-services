mod app;
mod influx_writer;
mod time;
mod writer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
