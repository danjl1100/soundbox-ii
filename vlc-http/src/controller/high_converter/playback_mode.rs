// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{ConverterIterator, LowAction};
use crate::controller::{LowCommand, PlaybackStatus, RepeatMode};

#[derive(Debug)]
pub struct Command {
    pub repeat: RepeatMode,
    pub random: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converter;
impl<'a> ConverterIterator<'a> for Converter {
    type Status = &'a PlaybackStatus;
    type Command = Command;
    fn next(&mut self, status: &PlaybackStatus, command: &Command) -> Result<(), LowAction> {
        let Command { repeat, random } = command;
        let change_loop = status.is_loop_all != repeat.is_loop_all();
        let change_repeat_one = status.is_repeat_one != repeat.is_repeat_one();
        let change_random = status.is_random != *random;
        match () {
            _ if change_loop => Err(LowCommand::ToggleLoopAll),
            _ if change_repeat_one => Err(LowCommand::ToggleRepeatOne),
            _ if change_random => Err(LowCommand::ToggleRandom),
            _ => Ok(()), // base case, matches desired state
        }
        .map_err(LowAction::from)
    }
}
