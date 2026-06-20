use beet_pusher::pipe_exec::NodePath;

use crate::domain::services::ports::BeetPusherPipe;

pub struct NodeService<T: BeetPusherPipe> {
    pipe: T,
}
impl<T: BeetPusherPipe> NodeService<T> {
    pub async fn create_bucket(parent: NodePath) -> Result<NodePath, CreateBucketError> {
        // TODO actually implement the service
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CreateBucketError {
    message: String,
}
