use anyhow::Result;
use async_trait::async_trait;
use hs_logger_core::{DataPointList, PointWriter};
use tracing::{debug, info};

#[derive(Default)]
pub struct LoggingPointWriter;

#[async_trait]
impl PointWriter for LoggingPointWriter {
    async fn write_points(&self, point_list: DataPointList) -> Result<()> {
        info!(count = point_list.len(), "logger writer received points");
        for point in point_list {
            debug!(
                measurement = %point.measurement,
                tag_count = point.tags.len(),
                field_count = point.fields.len(),
                observed_ms = point.observed_ms,
                "timeseries point"
            );
        }

        Ok(())
    }
}
