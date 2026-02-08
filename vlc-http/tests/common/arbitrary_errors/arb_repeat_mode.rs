use vlc_http::goal::RepeatMode;

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub enum ArbRepeatMode {
    Off,
    All,
    One,
}
impl From<RepeatMode> for ArbRepeatMode {
    fn from(value: RepeatMode) -> Self {
        match value {
            RepeatMode::Off => Self::Off,
            RepeatMode::All => Self::All,
            RepeatMode::One => Self::One,
        }
    }
}
impl From<ArbRepeatMode> for RepeatMode {
    fn from(value: ArbRepeatMode) -> Self {
        match value {
            ArbRepeatMode::Off => Self::Off,
            ArbRepeatMode::All => Self::All,
            ArbRepeatMode::One => Self::One,
        }
    }
}
