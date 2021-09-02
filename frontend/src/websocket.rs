use crate::Time;
use backoff::backoff::Backoff;
use gloo_timers::callback::Timeout;
use serde::{de::DeserializeOwned, Serialize};
use std::convert::TryInto;
use std::marker::PhantomData;
use yew::format::Json;
use yew::prelude::{Callback, ShouldRender};
use yew::services::websocket::{WebSocketError, WebSocketService, WebSocketStatus, WebSocketTask};

#[derive(Debug)]
pub(crate) struct Notify(NotifyMsg);
/// Privacy layer, to ensure all "status" events originate from this module
#[derive(Debug)]
struct NotifyMsg(WebSocketStatus);

pub(crate) struct Helper<T: Serialize, U: DeserializeOwned, B: Backoff> {
    url: String,
    on_message: Callback<Json<anyhow::Result<U>>>,
    on_notification: Callback<Notify>,
    /// Websocket task, and server heartbeat
    task: Option<(SocketTask<T, U>, Option<Time>)>,
    reconnector: ReconnectLogic<B>,
}
impl<T: Serialize, U: DeserializeOwned + 'static, B: Backoff> Helper<T, U, B> {
    pub(crate) fn new(
        url: String,
        on_message: &Callback<Result<U, anyhow::Error>>,
        on_notification: Callback<Notify>,
        reconnect: Callback<()>,
        reconnect_backoff: B,
    ) -> Self {
        let on_message = on_message.reform(|Json(message)| message);
        let reconnector = ReconnectLogic {
            timeout_millis: None,
            backoff: reconnect_backoff,
            reconnect,
        };
        Self {
            url,
            on_message,
            on_notification,
            task: None,
            reconnector,
        }
    }
    pub(crate) fn connect(&mut self) -> Result<(), WebSocketError> {
        self.reconnector.clear_timeout();
        let ignore = |_| ();
        self.create_task().map(ignore)
    }
    pub(crate) fn is_started(&self) -> bool {
        self.task.is_some()
    }
    pub(crate) fn is_connected(&self) -> bool {
        self.task
            .as_ref()
            .map_or(false, |(_, heartbeat)| heartbeat.is_some())
    }
    pub(crate) fn last_heartbeat(&self) -> Option<Time> {
        match &self.task {
            Some((_, Some(heartbeat))) => Some(*heartbeat),
            _ => None,
        }
    }
    pub(crate) fn get_reconnect_timeout_millis(&self) -> Option<u32> {
        self.reconnector.get_timeout_millis()
    }
    pub(crate) fn get_task(&mut self) -> Option<&mut SocketTask<T, U>> {
        self.task.as_mut().map(|(task, _)| task)
    }
    fn create_task(&mut self) -> Result<&mut SocketTask<T, U>, WebSocketError> {
        let on_notification = self
            .on_notification
            .clone()
            .reform(|event| Notify(NotifyMsg(event)));
        let task =
            WebSocketService::connect_text(&self.url, self.on_message.clone(), on_notification)?;
        self.task.replace((SocketTask::new(task), None));
        Ok(self.get_task().expect("replaced `task` option is some"))
    }
    pub(crate) fn on_notify(&mut self, Notify(NotifyMsg(event)): Notify) -> ShouldRender {
        let should_render = match event {
            WebSocketStatus::Closed | WebSocketStatus::Error => {
                self.task = None;
                true
            }
            WebSocketStatus::Opened => false,
        };
        if self.is_started() {
            self.reconnector.clear_timeout();
        } else {
            self.reconnector.set_timeout();
        }
        should_render
    }
    pub(crate) fn on_message(&mut self) {
        self.reconnector.reset_all();
        if let Some((_, heartbeat)) = &mut self.task {
            *heartbeat = Some(chrono::Utc::now());
        }
    }
}
pub(crate) struct SocketTask<T: Serialize, U: DeserializeOwned> {
    task: WebSocketTask,
    _phantom: PhantomData<dyn Fn(T) -> U>,
}
impl<T: Serialize, U: DeserializeOwned> SocketTask<T, U> {
    fn new(task: WebSocketTask) -> Self {
        Self {
            task,
            _phantom: PhantomData,
        }
    }
    pub(crate) fn send(&mut self, message: &T) {
        self.task.send(Json(message));
    }
}

struct ReconnectLogic<B: Backoff> {
    timeout_millis: Option<(Timeout, u32)>,
    backoff: B,
    reconnect: Callback<()>,
}
impl<B: Backoff> ReconnectLogic<B> {
    fn clear_timeout(&mut self) {
        self.timeout_millis = None;
    }
    fn reset_all(&mut self) {
        self.clear_timeout();
        self.backoff.reset();
    }
    fn get_timeout_millis(&self) -> Option<u32> {
        self.timeout_millis.as_ref().map(|(_, millis)| *millis)
    }
    fn set_timeout(&mut self) {
        if self.timeout_millis.is_some() {
            return;
        }
        if let Some(delay) = self.backoff.next_backoff() {
            let delay_millis = delay.as_millis().try_into().unwrap_or(u32::MAX);
            debug!("reconnect delay_millis = {}", delay_millis);
            let reconnect = self.reconnect.clone();
            let timeout = Timeout::new(delay_millis, move || {
                reconnect.emit(());
            });
            self.timeout_millis = Some((timeout, delay_millis));
        }
    }
}
