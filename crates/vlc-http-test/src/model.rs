// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Model for the VLC client state

/// Model of a VLC client instance, receiving raw commands from HTTP
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Model {
    items_created: u16,
    items: Vec<Item>,
    repeat_mode: RepeatMode,
    is_random: bool,
    art_endpoints: Vec<String>,
    current_item_id: Option<(i32, PlayState)>,
}
/// Serializable representation of [`Model`]
#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename = "Model")]
pub struct ModelJson<'a> {
    #[serde(skip)]
    items_created: u16,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    #[serde(serialize_with = "serialize_items_slice")]
    items: std::borrow::Cow<'a, [Item]>,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_loop_all: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_repeat_one: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_random: bool,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    art_endpoints: std::borrow::Cow<'a, [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_item_id: Option<(i32, PlayState)>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
enum RepeatMode {
    #[default]
    None,
    LoopAll,
    RepeatOne,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[expect(missing_docs, reason = "self-explanatory")]
pub enum PlayState {
    Playing,
    Paused,
}
/// Item in the [`Model`] playlist
#[derive(Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "self-explanatory")]
pub struct Item {
    pub id: i32,
    pub uri: String,
}

impl Model {
    /// Initializes the items in the playlist
    ///
    /// # Errors
    /// Returns an error if there are already items present
    pub fn initialize_items(&mut self, items: Vec<impl ToString>) -> Result<(), ItemsCreatedError> {
        let Self { items_created, .. } = *self;
        if items_created != 0 {
            return Err(ItemsCreatedError { items_created });
        }

        for item in items {
            self.push_uri(item.to_string());
        }
        Ok(())
    }

    /// Returns the current items
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }
    /// Sets the current playing item
    pub fn set_current_playing(&mut self, item_id: i32, state: PlayState) {
        dbg!((item_id, state));
        self.current_item_id = Some((item_id, state));
    }

    /// Returns the current playing item ID and state
    #[must_use]
    pub fn get_current_playing(&self) -> Option<(i32, PlayState)> {
        self.current_item_id
    }

    /// Returns a view of the current playlist items
    #[must_use]
    pub fn get_items(&self) -> &[Item] {
        &self.items
    }

    /// Returns a serializable view
    #[must_use]
    pub fn as_json(&self) -> ModelJson<'_> {
        let Self {
            items_created,
            ref items,
            repeat_mode: _, // accessor methods used instead
            is_random,
            ref art_endpoints,
            current_item_id,
        } = *self;
        ModelJson {
            items_created,
            items: items.into(),
            is_loop_all: self.is_loop_all(),
            is_repeat_one: self.is_repeat_one(),
            is_random,
            art_endpoints: art_endpoints.into(),
            current_item_id,
        }
    }

    /// Returns the track that is logically "after the current playing track"
    #[must_use]
    pub fn get_available_next_track(&self) -> Option<&Item> {
        if let Some((current_id, _)) = self.current_item_id {
            dbg!(&self.items);

            let mut found_current = false;
            self.items.iter().find(|item| {
                if found_current {
                    // after current
                    return true;
                }

                if item.id == current_id {
                    found_current = true;
                }
                false
            })
        } else {
            self.items.first()
        }
    }
}
impl ModelJson<'_> {
    /// Converts to an owned (`'static`) view
    #[must_use]
    pub fn clone_to_owned(&self) -> ModelJson<'static> {
        let ModelJson {
            items_created,
            ref items,
            is_loop_all,
            is_repeat_one,
            is_random,
            ref art_endpoints,
            current_item_id,
        } = *self;
        ModelJson {
            items_created,
            items: items.clone().into_owned().into(),
            is_loop_all,
            is_repeat_one,
            is_random,
            art_endpoints: art_endpoints.clone().into_owned().into(),
            current_item_id,
        }
    }
}
/// Error from [`Model::initialize_items`]
#[derive(Debug)]
pub struct ItemsCreatedError {
    items_created: u16,
}
impl std::error::Error for ItemsCreatedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
impl std::fmt::Display for ItemsCreatedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { items_created } = self;
        write!(
            f,
            "cannot initialize_items, already processed {items_created} items"
        )
    }
}

/// Response from [`Model::request`]
#[derive(Debug)]
pub enum ModelResponse {
    /// Raw JSON simulated VLC response
    Json(String),
    /// Placeholder for the art endpoint binary data
    Art,
}

impl Model {
    /// Modifies the model based on the endpoint URL
    ///
    /// # Errors
    /// Returns an error if the request endpoint is not valid
    pub fn request(&mut self, endpoint: &str) -> Result<ModelResponse, RequestError> {
        // FIXME improve parsing strategy
        let (path, args) =
            endpoint
                .split_once('?')
                .map_or((endpoint, Vec::new()), |(base, args)| {
                    (
                        base,
                        args.split('&')
                            .map(|arg| {
                                arg.split_once('=')
                                    .map_or((arg, None), |(key, val)| (key, Some(val)))
                            })
                            .collect(),
                    )
                });

        let command = args
            .iter()
            .find_map(|&(key, val)| (key == "command").then_some(val).flatten());
        let args: Vec<_> = args
            .into_iter()
            .filter(|&(key, _val)| key != "command")
            .collect();

        let response = if path == "/requests/playlist.json" {
            match command {
                Some("in_enqueue") => self.enqueue(&args),
                Some("pl_delete") => self.delete(&args),
                Some(command) => Err(RequestErrorKind::unknown_command("playlist", command)),
                None => Ok(self.get_playlist_info()),
            }
            .map(ModelResponse::Json)
        } else if path == "/requests/status.json" {
            if args.is_empty() {
                match command {
                    None => Ok(self.get_playback_status()),
                    Some("pl_random") => Ok(self.toggle_random()),
                    Some("pl_loop") => Ok(self.toggle_loop_all()),
                    Some("pl_repeat") => Ok(self.toggle_repeat_one()),
                    Some("pl_forcepause") => Ok(self.set_playing_paused()),
                    Some("pl_forceresume") => Ok(self.set_playing_resume()),
                    Some(command) => Err(RequestErrorKind::unknown_command(
                        "playback no-arg",
                        command,
                    )),
                }
                .map(ModelResponse::Json)
            } else {
                match command {
                    None => Err(RequestErrorKind::invalid_args("no-command has args", &args)),
                    Some("pl_play") => self.play(&args),
                    Some(command) => Err(RequestErrorKind::unknown_command_args(
                        "playback", command, &args,
                    )),
                }
                .map(ModelResponse::Json)
            }
        } else if path == "/art" {
            self.art_endpoints.push(endpoint.to_string());
            Ok(ModelResponse::Art)
        } else {
            Err(RequestErrorKind::UnknownEndpoint)
        };

        response.map_err(|kind| RequestError {
            kind,
            endpoint: endpoint.to_string(),
        })
    }
}

/// Error from [`Model::request`]
#[derive(Debug)]
pub struct RequestError {
    kind: RequestErrorKind,
    endpoint: String,
}
impl std::error::Error for RequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let Self { kind, endpoint: _ } = self;
        Some(kind)
    }
}
impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind: _, endpoint } = self;
        write!(f, "failed to parse endpoint: {endpoint:?}")
    }
}

#[derive(Debug)]
enum RequestErrorKind {
    InvalidArgs {
        operation: &'static str,
        args: Vec<(String, Vec<String>)>,
    },
    InvalidId {
        id_str: String,
        source: std::num::ParseIntError,
    },
    UnknownCommand {
        kind: &'static str,
        command: String,
        args: Option<Vec<(String, Vec<String>)>>,
    },
    UnknownEndpoint,
}
impl RequestErrorKind {
    fn invalid_args(operation: &'static str, args: &[(&str, Option<&str>)]) -> Self {
        let args = Self::collect_args(args);
        Self::InvalidArgs { operation, args }
    }

    fn unknown_command(kind: &'static str, command: &str) -> RequestErrorKind {
        Self::UnknownCommand {
            kind,
            command: command.to_string(),
            args: None,
        }
    }
    fn unknown_command_args(
        kind: &'static str,
        command: &str,
        args: &[(&str, Option<&str>)],
    ) -> RequestErrorKind {
        let args = Self::collect_args(args);
        Self::UnknownCommand {
            kind,
            command: command.to_string(),
            args: Some(args),
        }
    }
    fn collect_args(args: &[(&str, Option<&str>)]) -> Vec<(String, Vec<String>)> {
        args.iter()
            .map(|(key, values)| {
                (
                    key.to_string(),
                    values
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                )
            })
            .collect()
    }
}
impl std::error::Error for RequestErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidId { source, .. } => Some(source),
            Self::InvalidArgs {
                operation: _,
                args: _,
            }
            | Self::UnknownCommand {
                kind: _,
                command: _,
                args: _,
            }
            | Self::UnknownEndpoint => None,
        }
    }
}
impl std::fmt::Display for RequestErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RequestErrorKind as Kind;
        match self {
            Kind::InvalidArgs { operation, args } => {
                write!(f, "invalid args for {operation:?}: {args:?}")
            }
            Kind::UnknownCommand {
                kind,
                command,
                args,
            } => write!(f, "unknown {kind:?} command {command:?}: {args:?}"),
            Kind::UnknownEndpoint => write!(f, "unknown endpoint"),
            Kind::InvalidId { id_str, source: _ } => write!(f, "invalid ID string: {id_str:?}"),
        }
    }
}

impl Model {
    fn enqueue(&mut self, args: &[(&str, Option<&str>)]) -> Result<String, RequestErrorKind> {
        let [("input", Some(val))] = *args else {
            return Err(RequestErrorKind::invalid_args("enqueue", args));
        };

        let uri = urlencoding::decode(val).expect("invalid URI").to_string();

        self.push_uri(uri);

        Ok(self.get_playlist_info())
    }
    fn push_uri(&mut self, uri: String) {
        let items_created: u16 = self.items_created;
        let id: i32 = items_created.into();
        self.items_created = self.items_created.checked_add(1).expect("max URIs pushed");

        self.items.push(Item { id, uri });
    }
    fn delete(&mut self, args: &[(&str, Option<&str>)]) -> Result<String, RequestErrorKind> {
        let [("id", Some(val))] = *args else {
            return Err(RequestErrorKind::invalid_args("delete", args));
        };

        let id: i32 = val
            .parse::<i32>()
            .map_err(|source| RequestErrorKind::InvalidId {
                id_str: val.to_string(),
                source,
            })?;

        self.items.retain(|item| item.id != id);

        Ok(self.get_playlist_info())
    }
    fn play(&mut self, args: &[(&str, Option<&str>)]) -> Result<String, RequestErrorKind> {
        let [("id", Some(val))] = *args else {
            return Err(RequestErrorKind::invalid_args("play", args));
        };

        let id: i32 = val
            .parse::<i32>()
            .map_err(|source| RequestErrorKind::InvalidId {
                id_str: val.to_string(),
                source,
            })?;

        self.current_item_id = Some((id, PlayState::Playing));

        Ok(self.get_playback_status())
    }

    fn is_loop_all(&self) -> bool {
        matches!(self.repeat_mode, RepeatMode::LoopAll)
    }
    fn is_repeat_one(&self) -> bool {
        matches!(self.repeat_mode, RepeatMode::RepeatOne)
    }

    fn toggle_random(&mut self) -> String {
        self.is_random = !self.is_random;
        self.get_playback_status()
    }
    fn toggle_loop_all(&mut self) -> String {
        self.repeat_mode = if self.is_loop_all() {
            RepeatMode::None
        } else {
            RepeatMode::LoopAll
        };
        self.get_playback_status()
    }
    fn toggle_repeat_one(&mut self) -> String {
        self.repeat_mode = if self.is_repeat_one() {
            RepeatMode::None
        } else {
            RepeatMode::RepeatOne
        };
        self.get_playback_status()
    }
    fn set_playing_paused(&mut self) -> String {
        if let Some((id, state)) = self.current_item_id {
            let new_state = match state {
                PlayState::Playing | PlayState::Paused => PlayState::Paused,
            };
            self.current_item_id = Some((id, new_state));
        }
        self.get_playback_status()
    }
    fn set_playing_resume(&mut self) -> String {
        if let Some((id, state)) = self.current_item_id {
            let new_state = match state {
                PlayState::Playing | PlayState::Paused => PlayState::Playing,
            };
            self.current_item_id = Some((id, new_state));
        }
        self.get_playback_status()
    }

    fn get_playlist_info(&self) -> String {
        let items = self
            .items
            .iter()
            .map(|Item { id, uri }| {
                serde_json::json!({
                    // arbitrary (deterministic)
                    "duration": id*100+(7*(id % 3)),
                    "uri": uri,
                    "type": "leaf",
                    "id": id.to_string(),
                    "ro": "rw",
                    "name": format!("Item {id}"),
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "children":[{
                "children": items,
                "name":"Playlist",
            }]
        })
        .to_string()
    }

    fn get_playback_status(&self) -> String {
        serde_json::json!({
            "rate":1,
            "time":0,
            "repeat": self.is_repeat_one(),
            "loop": self.is_loop_all(),
            "length":0,
            "random": self.is_random,
            "apiversion":3,
            "version":"3.0.20 Vetinari",
            "currentplid":self.current_item_id.map_or(-1, |(id, _)| id),
            "position":0.0,
            "volume":256,
            "state":"playing", // TODO: paused, stopped, playing test them all!
            "information":{"category":{"meta":{}}},
        })
        .to_string()
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "signature required by serde"
)]
fn bool_is_false(value: &bool) -> bool {
    !(*value)
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // force single-line for cleanliness
        let Self { id, uri } = self;
        write!(f, "{id}: {uri}")
    }
}
fn serialize_items_slice<S>(items: &[Item], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap as _;

    // NOTE: assumes all IDs are unique (which they *should* be)
    let len = Some(items.len());
    let mut seq = serializer.serialize_map(len)?;
    for item in items {
        let Item { id, uri } = item;
        seq.serialize_key(&id)?;
        seq.serialize_value(&uri)?;
    }
    seq.end()
}
