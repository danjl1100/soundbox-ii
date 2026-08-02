// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Logic for manipulating nodes in the `bucket_spigot` network

use beet_pusher::pipe_exec::{Command, NodePath, ResponseData, SpigotCmd};

use crate::domain::services::ports::BeetPusherPipe;

const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);
/// Controls nodes in the `bucket_spigot` network
pub struct NodeService<T> {
    pipe: T,
}
impl<T: BeetPusherPipe> NodeService<T> {
    pub(crate) fn new(pipe: T) -> Self {
        Self { pipe }
    }

    /// Creates a bucket at the specified path
    ///
    /// # Errors
    /// Returns an error if sending the request fails, or the response is incorrect
    pub async fn create_bucket(&self, parent: NodePath) -> Result<NodePath, CreateBucketError> {
        let cmd = Command::Spigot(SpigotCmd::AddNode {
            parent,
            node_kind: beet_pusher::pipe_exec::NodeKind::Bucket,
        });
        let response = self
            .pipe
            .send(cmd, TIMEOUT)
            .await
            .map_err(Into::into)
            .map_err(CreateBucketError::Unknown)?;

        let path = match response {
            ResponseData::NodeAdded { path } => path,
            found @ ResponseData::PassNoData => {
                return Err(CreateBucketError::ExpectedNodeAdded { found });
            }
        };

        Ok(path)
    }
}

/// Error creating a bucket
#[derive(Debug, thiserror::Error)]
pub enum CreateBucketError {
    /// Mismatched response, unable to retrieve the created bucket path
    #[error("expected NodeAdded response, found: {found:?}")]
    ExpectedNodeAdded {
        /// mismatched response
        found: ResponseData,
    },
    /// Backend-specific error
    #[error(transparent)]
    Unknown(#[from] eyre::Error),
}
