//! Miscellaneous formatting functions

pub(crate) fn fmt_duration_seconds(seconds: u64) -> String {
    let (hour, min, sec) = seconds_to_hms(seconds);
    if hour == 0 {
        format!("{}:{:02}", min, sec)
    } else {
        format!("{}:{:02}:{:02}", hour, min, sec)
    }
}

pub(crate) fn fmt_duration_seconds_long(seconds: u64) -> String {
    let (hour, min, sec) = seconds_to_hms(seconds);
    if hour == 0 {
        if min == 0 {
            format!("{} seconds", sec)
        } else {
            format!("{} minutes {} seconds", min, sec)
        }
    } else {
        format!("{} hours {} minutes {} seconds", hour, min, sec)
    }
}

fn seconds_to_hms(seconds: u64) -> (u64, u64, u64) {
    let hour = (seconds / 60) / 60;
    let min = (seconds / 60) % 60;
    let sec = seconds % 60;
    (hour, min, sec)
}
