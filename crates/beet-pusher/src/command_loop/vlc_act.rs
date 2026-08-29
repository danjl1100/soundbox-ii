// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crossbeam_channel::SendError;

use vlc_http::goal::TargetPlaylistItems;
use vlc_http_ureq::HttpRunner;

use crate::{
    PlaylistUpdateCountsResult, VlcDriver,
    command_loop::{LoopEvent, LoopEventVlcCmd},
    pipe_exec::VlcCmd,
};

pub struct PlaylistResponse<E>(pub mut(self) PlaylistUpdateCountsResult<E>);
impl<E> std::fmt::Debug for PlaylistResponse<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            Ok(counts) => f.debug_tuple("PlaylistResponse::Ok").field(counts).finish(),
            Err(_) => f
                .debug_tuple("PlaylistResponse::Err")
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug)]
enum Action {
    MaintainPlaylist(TargetPlaylistItems),
    Command(LoopEventVlcCmd),
}

pub struct Sender {
    tx: std::sync::mpsc::SyncSender<Action>,
}
impl Sender {
    pub fn send_playlist_update_action(
        &self,
        playlist_items: TargetPlaylistItems,
    ) -> Result<(), SenderError> {
        self.tx
            .try_send(Action::MaintainPlaylist(playlist_items))
            .map_err(SenderError)
    }
    pub fn send_cmd(&self, cmd: LoopEventVlcCmd) -> Result<(), SenderError> {
        self.tx.try_send(Action::Command(cmd)).map_err(SenderError)
    }
}

/// Spawns a thread to handle VLC requests
pub fn spawn(
    loop_tx: crossbeam_channel::Sender<LoopEvent>,
    mut vlc_driver: VlcDriver<HttpRunner>,
) -> (Sender, JoinHandle) {
    const CHANNEL_DEPTH: usize = 4;
    let (tx, mut rx) = std::sync::mpsc::sync_channel(CHANNEL_DEPTH);

    let sender = Sender { tx };
    let handle = std::thread::spawn(move || match run(&mut rx, &loop_tx, &mut vlc_driver) {
        Ok(()) => {}
        Err(SendError(_event)) => {
            // NOTE: nothing to do, main event loop is no longer receiving
        }
    });
    (sender, JoinHandle(handle))
}

fn run(
    rx: &mut std::sync::mpsc::Receiver<Action>,
    loop_tx: &crossbeam_channel::Sender<LoopEvent>,
    vlc_driver: &mut VlcDriver<HttpRunner>,
) -> Result<(), SendError<LoopEvent>> {
    while let Ok(action) = rx.recv() {
        match action {
            Action::MaintainPlaylist(target) => {
                let result = vlc_driver.run_playlist_update_action(target);
                loop_tx.send(LoopEvent::MaintainPlaylistResponse(PlaylistResponse(
                    result,
                )))?;
            }
            Action::Command(cmd) => {
                let modifies_playlist = match &cmd.cmd {
                    VlcCmd::SeekNext => true,
                };

                cmd.run(vlc_driver);

                // send the `modifies_playlist` hint **after** the command finishes
                loop_tx.send(LoopEvent::VlcCmdResponse { modifies_playlist })?;
            }
        }
    }
    Ok(())
}

pub struct JoinHandle(std::thread::JoinHandle<()>);
impl JoinHandle {
    pub fn join_and_drop(self, sender: Sender) -> std::thread::Result<()> {
        drop(sender);

        let Self(handle) = self;
        handle.join()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to queue to VLC action thread")]
pub struct SenderError(#[source] std::sync::mpsc::TrySendError<Action>);
