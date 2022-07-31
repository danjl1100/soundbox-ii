// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use yew::{html, Component, Context, Html};

use crate::macros::UpdateDelegate;

shared::wrapper_enum! {
    pub(crate) enum Msg {
        ClearErrors(ClearErrors),
        { impl None for }
        Message(String),
        Error((&'static str, String)),
        ShowErrorDetails(bool),
    }
}
pub struct ClearErrors {
    _seal: (),
}
#[derive(Default)]
pub(crate) struct Logger {
    errors: Vec<LogErrorEntry>,
    show_error_details: bool,
}
impl Logger {
    pub(crate) fn error_view<C>(&self, ctx: &Context<C>) -> Html
    where
        C: Component,
        <C as Component>::Message: From<Msg>,
    {
        let show_details = self.show_error_details;
        let link = ctx.link();
        let toggle_details = link.callback(move |_| Msg::ShowErrorDetails(!show_details));
        let clear = link.callback(|_| Msg::from(ClearErrors { _seal: () }));
        html! {
            <div>
                if self.errors.is_empty() {
                    { "No Errors" }
                } else {
                    <>
                        { self.errors.len() }
                        { " Errors " }
                        <button onclick={toggle_details}>
                            { if show_details { "Hide" } else { "Show" } }
                        </button>
                        { " " }
                        <button onclick={clear}>{ "Clear" }</button>
                        if show_details {
                            { self.errors.iter().collect::<Html>() }
                        }
                    </>
                }
            </div>
        }
    }
    fn clear_errors(&mut self) -> bool {
        let was_empty = self.errors.is_empty();
        self.errors.clear();
        !was_empty
    }
    fn set_show_error_details(&mut self, show: bool) -> bool {
        let changed = self.show_error_details != show;
        self.show_error_details = show;
        changed
    }
}
struct LogErrorEntry(&'static str, String);
impl<'a> FromIterator<&'a LogErrorEntry> for Html {
    fn from_iter<T: IntoIterator<Item = &'a LogErrorEntry>>(iter: T) -> Self {
        html! {
            <ul class="errors">
                {
                    iter.into_iter().enumerate().map(|(index, LogErrorEntry(ty, msg))| html! {
                        <li key={index}>
                            <b>{ty}</b>
                            {" - "}
                            { msg }
                        </li>
                    }).collect::<Html>()
                }
            </ul>
        }
    }
}
impl<C> UpdateDelegate<C> for Logger
where
    C: Component,
{
    type Message = Msg;

    fn update(&mut self, _ctx: &Context<C>, message: Self::Message) -> bool {
        match message {
            Msg::Message(message) => {
                log!("App got message {}", message);
                false
            }
            Msg::Error((error_ty, error_msg)) => {
                self.errors.push(LogErrorEntry(error_ty, error_msg));
                true
            }
            Msg::ClearErrors(..) => {
                let errors = self.clear_errors();
                let show = self.set_show_error_details(false);
                errors || show
            }
            Msg::ShowErrorDetails(show) => self.set_show_error_details(show),
        }
    }
}
