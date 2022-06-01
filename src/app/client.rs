//! Client-specific logic

use std::sync::Arc;

use super::App;
use anyhow::Result;
use matrix_sdk::{
    ruma::api::{
        client::{error::ErrorKind, uiaa::UiaaResponse},
        error::{FromHttpResponseError, ServerError},
    },
    Client, HttpError,
};
use twilight_model::id::{marker::UserMarker, Id};

impl App {
    /// Attempts to register a new user
    pub(super) async fn try_register_user(
        self: &Arc<Self>,
        localpart: impl AsRef<str> + Send + Sync,
    ) -> Result<()> {
        match self.appservice.register_virtual_user(localpart).await {
            Err(matrix_sdk_appservice::Error::Matrix(matrix_sdk::Error::Http(
                HttpError::UiaaError(FromHttpResponseError::Server(ServerError::Known(
                    UiaaResponse::MatrixError(error),
                ))),
            ))) if matches!(error.kind, ErrorKind::UserInUse) => Ok(()),
            r => Ok(r?),
        }
    }

    /// Returns a client for user ID
    ///
    /// # Errors
    /// This function will return an error if retrieving the client fails
    pub async fn client(self: &Arc<Self>, user_id: Option<Id<UserMarker>>) -> Result<Arc<Client>> {
        match user_id {
            None => Ok(Arc::clone(&self.client)),
            Some(user_id) => {
                if let Some(client) = self.discord_clients.get(&user_id) {
                    Ok(Arc::clone(&*client))
                } else {
                    let username = format!("{}_discord_{}", self.config.bridge.prefix, user_id);
                    self.try_register_user(&username).await?;
                    let user = Arc::new(self.appservice.virtual_user_client(&username).await?);
                    self.discord_clients.insert(user_id, Arc::clone(&user));
                    Ok(user)
                }
            }
        }
    }
}
