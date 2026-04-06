use anyhow::Result;
use async_trait::async_trait;

use crate::datapoint::DataPointList;

#[async_trait]
pub trait PointWriter: Send + Sync {
    async fn write_points(&self, point_list: DataPointList) -> Result<()>;
}
