// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{
    command::LowCommand,
    controller::{ret, Channels, Controller, HighCommand},
    vlc_responses, Error,
};

mod playback_mode;
mod playlist_set;

shared::wrapper_enum! {
    #[derive(Clone, Debug, PartialEq)]
    pub(super) enum LowAction {
        Command(LowCommand),
        { impl None for }
        QueryPlaybackStatus,
        QueryPlaylistInfo,
    }
}

trait ConverterIterator<'a> {
    type Status;
    type Command;
    fn next(&mut self, status: Self::Status, command: &Self::Command) -> Result<(), LowAction>;
}

#[derive(Debug)]
pub(super) enum State {
    PlaybackMode {
        converter: playback_mode::Converter,
        command: playback_mode::Command,
    },
    PlaylistSet {
        converter: playlist_set::Converter,
        command: playlist_set::Command,
    },
}
impl From<HighCommand> for State {
    fn from(command: HighCommand) -> Self {
        match command {
            HighCommand::PlaylistSet {
                urls,
                max_history_count,
            } => Self::PlaylistSet {
                converter: playlist_set::Converter::new(),
                command: playlist_set::Command {
                    urls,
                    max_history_count,
                },
            },
            HighCommand::PlaybackMode { repeat, random } => Self::PlaybackMode {
                converter: playback_mode::Converter,
                command: playback_mode::Command { repeat, random },
            },
        }
    }
}
impl State {
    pub async fn run(mut self, controller: &mut Controller) -> Result<(), Error> {
        let mut runaway_counter = None;
        let mut count = 0;
        while let Err(low_action) = self.next(&mut controller.channels, &()) {
            {
                //TODO remove training-wheels (When you are Ready,  use the force,  etc.)
                assert!(count < 200, "exceeded training-wheels counter {count}");
                count += 1;
                runaway_counter = match runaway_counter.take() {
                    Some((count, prev_action)) if count > 10 => {
                        return Err(Error::Logic(format!(
                            "runaway while executing {self:?}, repeated {count} times: {prev_action:?}"
                        )));
                    }
                    Some((count, prev_action)) if prev_action == low_action => {
                        Some((count + 1, prev_action))
                    }
                    None => Some((0, low_action.clone())),
                    Some(_) => None,
                };
            }
            match low_action {
                LowAction::Command(low_command) => {
                    controller.run_low_command(low_command).await?;
                }
                LowAction::QueryPlaybackStatus => {
                    controller.run_query_playback_status::<ret::None>().await?;
                }
                LowAction::QueryPlaylistInfo => {
                    controller.run_query_playlist_info::<ret::None>().await?;
                }
            }
        }
        Ok(())
    }
}
macro_rules! with_define {
    (
        $(
            $macro_name:ident: $value_ty:ty =>
            const $err:ident: $err_ty:ty = $err_val:expr;
        )+
    ) => {
        $(
            const $err: $err_ty = $err_val;
            macro_rules! $macro_name {
                (if let Ok($dest:ident) = borrow($field:expr) $content:expr) => {
                    (&*$field.borrow())
                        .as_ref()
                        .ok_or($err)
                        .and_then(|$dest: &$value_ty| $content)
                };
            }
        )+
    };
}
with_define! {
    with_status: vlc_responses::PlaybackStatus =>
    const QUERY_PLAYBACK_STATUS: LowAction = LowAction::QueryPlaybackStatus;
    with_playlist: vlc_responses::PlaylistInfo =>
    const QUERY_PLAYLIST_INFO: LowAction = LowAction::QueryPlaylistInfo;
}
impl<'a> ConverterIterator<'a> for State {
    type Status = &'a Channels;
    type Command = ();
    fn next(&mut self, channels: &'a Channels, _: &()) -> Result<(), LowAction> {
        match self {
            State::PlaybackMode { converter, command } => {
                with_status!(if let Ok(status) = borrow(channels.playback_status_tx) {
                    converter.next(status, command)
                })
            }

            State::PlaylistSet { converter, command } => {
                with_status!(if let Ok(status) = borrow(channels.playback_status_tx) {
                    with_playlist!(if let Ok(playlist) = borrow(channels.playlist_info_tx) {
                        converter.next((status, playlist), command)
                    })
                })
            }
        }
    }
}
