use crate::domain::services::ports::BeetPusherPipe;

pub struct StdIoPipe {}
impl BeetPusherPipe for StdIoPipe {
    type Error = std::convert::Infallible;

    async fn send(
        &self,
        command: beet_pusher::pipe_exec::Command,
        timeout: std::time::Duration,
    ) -> Result<beet_pusher::pipe_exec::ResponseData, Self::Error> {
        todo!()
    }
}

// pub struct StdIoPipe {
//     commands_tx: tokio::sync::mpsc::Sender<beet_pusher::pipe_exec::Command>,
//     _results_rx: tokio::sync::broadcast::Receiver<()>,
// }
// impl StdIoPipe {
//     pub fn new() -> (Self, StdIoRunner) {
//         let (results_tx, results_rx) = tokio::sync::broadcast::channel(1);
//         let (commands_tx, commands_rx) = tokio::sync::mpsc::channel(1);
//         let pipe = Self {
//             commands_tx,
//             _results_rx: results_rx,
//         };
//         let runner = StdIoRunner {
//             command_writer: CommandWriter { commands_rx },
//             result_reader: ResultReader { results_tx },
//         };
//         (pipe, runner)
//     }
// }
//
// pub struct StdIoRunner {
//     result_reader: ResultReader,
//     command_writer: CommandWriter,
// }
// struct ResultReader {
//     results_tx: tokio::sync::broadcast::Sender<()>,
// }
// struct CommandWriter {
//     commands_rx: tokio::sync::mpsc::Receiver<beet_pusher::pipe_exec::Command>,
// }
// impl StdIoRunner {
//     pub async fn run(self) -> std::io::Result<()> {
//         let Self {
//             result_reader,
//             command_writer,
//         } = self;
//         tokio::try_join!(
//             // TODO: read more often than writing?
//             result_reader.run(),
//             command_writer.run()
//         )
//         .map(|((), ())| ())
//     }
// }
// impl ResultReader {
//     async fn run(self) -> std::io::Result<()> {
//         todo!()
//     }
// }
// impl CommandWriter {
//     async fn run(mut self) -> std::io::Result<()> {
//         let stdout = tokio::io::stdout();
//         let stdout = tokio::io::BufWriter::new(stdout);
//         loop {
//             let Some(cmd) = self.commands_rx.recv().await else {
//                 return Ok(());
//             };
//             let src = serde_json::
//             stdout.write_all(src).await?;
//         }
//     }
// }
// impl BeetPusherPipe for StdIoPipe {
//     async fn send(
//         &self,
//         command: beet_pusher::pipe_exec::Command,
//         timeout: std::time::Duration,
//     ) -> AppResult<beet_pusher::pipe_exec::ResponseData> {
//         let mut rx = self.results_tx.subscribe();
//         let start = std::time::Instant::now();
//         loop {
//             rx.recv().await;
//             if start.elapsed() > timeout {
//                 break;
//             }
//         }
//         todo!()
//     }
// }
