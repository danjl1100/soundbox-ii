// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Miscellaneous formatting functions

pub(crate) fn fmt_duration_seconds(seconds: u64) -> String {
    let (hour, min, sec) = seconds_to_hms(seconds);
    if hour == 0 {
        format!("{min}:{sec:02}")
    } else {
        format!("{hour}:{min:02}:{sec:02}")
    }
}

pub(crate) fn fmt_duration_seconds_long(seconds: u64) -> String {
    let (hour, min, sec) = seconds_to_hms(seconds);
    if hour == 0 {
        if min == 0 {
            format!("{sec} seconds")
        } else {
            format!("{min} minutes {sec} seconds")
        }
    } else {
        format!("{hour} hours {min} minutes {sec} seconds")
    }
}

fn seconds_to_hms(seconds: u64) -> (u64, u64, u64) {
    let hour = (seconds / 60) / 60;
    let min = (seconds / 60) % 60;
    let sec = seconds % 60;
    (hour, min, sec)
}
