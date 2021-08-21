use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use yew::format::Json;
use yew::prelude::{Callback, ShouldRender};
use yew::services::websocket::{WebSocketError, WebSocketService, WebSocketStatus, WebSocketTask};

#[derive(Debug)]
pub(crate) struct Notify(NotifyMsg);
/// Privacy layer, to ensure all "status" events originate from this module
#[derive(Debug)]
struct NotifyMsg(WebSocketStatus);

pub(crate) struct Helper<T: Serialize, U: DeserializeOwned> {
    url: &'static str,
    on_message: Callback<Json<anyhow::Result<U>>>,
    on_notification: Callback<Notify>,
    task: Option<SocketTask<T, U>>,
}
pub(crate) struct SocketTask<T: Serialize, U: DeserializeOwned> {
    task: WebSocketTask,
    _phantom: PhantomData<dyn Fn(T) -> U>,
}
impl<T: Serialize, U: DeserializeOwned + 'static> Helper<T, U> {
    pub(crate) fn new(
        url: &'static str,
        on_message: &Callback<Result<U, anyhow::Error>>,
        on_notification: Callback<Notify>,
    ) -> Self {
        let on_message = on_message.reform(|Json(message)| message);
        Self {
            url,
            on_message,
            on_notification,
            task: None,
        }
    }
    pub(crate) fn connect(&mut self) -> Result<(), WebSocketError> {
        let ignore = |_| ();
        self.create_task().map(ignore)
    }
    pub(crate) fn is_connected(&self) -> bool {
        self.task.is_some()
    }
    pub(crate) fn get_task(&mut self) -> Option<&mut SocketTask<T, U>> {
        self.task.as_mut()
    }
    fn create_task(&mut self) -> Result<&mut SocketTask<T, U>, WebSocketError> {
        let on_notification = self
            .on_notification
            .clone()
            .reform(|event| Notify(NotifyMsg(event)));
        let task =
            WebSocketService::connect_text(self.url, self.on_message.clone(), on_notification)?;
        self.task.replace(SocketTask {
            task,
            _phantom: PhantomData,
        });
        Ok(self.task.as_mut().expect("replaced `task` option is some"))
    }
    pub(crate) fn on_notify(&mut self, Notify(NotifyMsg(event)): Notify) -> ShouldRender {
        match event {
            WebSocketStatus::Closed | WebSocketStatus::Error => {
                self.task = None;
                true
            }
            WebSocketStatus::Opened => false,
        }
    }
}
impl<T: Serialize, U: DeserializeOwned> SocketTask<T, U> {
    pub(crate) fn send(&mut self, message: &T) {
        self.task.send(Json(message));
    }
}
