// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Convenience functions for [`Pollable`] in a synchronous (blocking) context

use crate::{
    action::Poll, client_state::ClientStateSequence, ClientState, Endpoint, Pollable, Response,
};

/// IO portion that resolves [`Endpoint`]s into the [`Response`]
pub trait EndpointRequestor {
    /// Error for sending the request and parsing the response
    type Error;
    /// Request the specified [`Endpoint`] and return the parsed [`Response`]
    ///
    /// # Errors
    /// Returns an error when requesting the [`Endpoint`] or parsing the [`Response`] fails
    fn request(&mut self, endpoint: Endpoint) -> Result<Response, Self::Error>;
}

impl<F, E> EndpointRequestor for F
where
    F: FnMut(Endpoint) -> Result<Response, E>,
{
    type Error = E;
    fn request(&mut self, endpoint: Endpoint) -> Result<Response, Self::Error> {
        (self)(endpoint)
    }
}

/// Convenience function for running a [`Pollable`] to completion, blocking until the output is
/// obtained or an error occurs.
///
/// # Errors
/// Returns an error if the endpoint generation fails, `call_endpoint_fn` fails, or the
/// `max_iter_count` is exceeded.
///
///
/// NOTE: While an equivalent helper function could be created for `async` (accepting an `async`
/// closure) this is left for the user to implement, as they may need to select between other
/// competing futures.
#[allow(clippy::module_name_repetitions)]
pub fn exhaust_pollable<'a, T, E, F>(
    mut source: T,
    client_state: &'a mut ClientState,
    endpoint_caller: &mut F,
    max_iter_count: usize,
) -> Result<(T::Output<'a>, Option<ClientStateSequence>), Error<T, E>>
where
    T: Pollable,
    F: EndpointRequestor<Error = E>,
    Error<T, E>: std::error::Error,
{
    let inner = |source: &mut T, client_state: &'a mut ClientState, endpoint_caller: &mut F| {
        let mut last_state_sequence = None;
        // FIXME does a `loop` fix the borrowing issue? (currently duplicates final call to `next`)
        for _ in 0..max_iter_count {
            let Poll::Need(endpoint) = source.next(client_state).map_err(ErrorKind::Poll)? else {
                break; // final output borrow occurs below
            };
            let response = endpoint_caller
                .request(endpoint)
                .map_err(ErrorKind::EndpointFn)?;

            let seq = client_state.update(response);

            last_state_sequence = Some(seq);
        }
        match source.next(client_state).map_err(ErrorKind::Poll)? {
            Poll::Done(output) => Ok((output, last_state_sequence)),
            Poll::Need(next_endpoint) => Err(ErrorKind::IterationCountExceeded {
                max_iter_count,
                next_endpoint,
            }),
        }
    };
    inner(&mut source, client_state, endpoint_caller).map_err(|kind| Error { source, kind })
}

/// Failure to exhaust a [`Pollable`] to the final output
///
/// See [`exhaust_pollable`]
#[derive(Debug)]
pub struct Error<T, E> {
    source: T,
    kind: ErrorKind<E>,
}
#[derive(Debug)]
enum ErrorKind<E> {
    Poll(crate::action::Error),
    EndpointFn(E),
    IterationCountExceeded {
        max_iter_count: usize,
        next_endpoint: Endpoint,
    },
}
impl<T, E> std::error::Error for Error<T, E>
where
    T: std::fmt::Debug,
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Poll(error) => Some(error),
            ErrorKind::EndpointFn(error) => Some(error),
            ErrorKind::IterationCountExceeded { .. } => None,
        }
    }
}
impl<T, E> std::fmt::Display for Error<T, E>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { source, kind } = self;
        match kind {
            ErrorKind::Poll(_) => write!(f, "failed to determine next endpoint"),
            ErrorKind::EndpointFn(_) => write!(f, "failed evaluating endpoint"),
            ErrorKind::IterationCountExceeded{max_iter_count, next_endpoint} =>
            write!(f, "exceeded iteration count safety net ({max_iter_count}), next endpoint {next_endpoint:?}"),
        }?;
        write!(f, " for source {source:?}")
    }
}
