mod app;
mod influx_writer;
mod status;
mod tracking_processor;
mod time;
mod writer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
