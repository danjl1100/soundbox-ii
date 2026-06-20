use beet_pusher::pipe_exec::{Command, ResponseData};

pub trait BeetPusherPipe: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    fn send(
        &self,
        command: Command,
        timeout: std::time::Duration,
    ) -> impl Future<Output = Result<ResponseData, Self::Error>> + Send;
}
