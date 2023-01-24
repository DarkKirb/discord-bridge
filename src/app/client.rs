//! Client-specific logic

use std::{ops::Deref, sync::Arc, time::Duration};

use super::App;
use anyhow::Result;
use matrix_sdk::{
    config::SyncSettings,
    locks::Mutex,
    room::Room,
    ruma::{
        api::{
            client::{error::ErrorKind, uiaa::UiaaResponse},
            error::{FromHttpResponseError, ServerError},
        },
        RoomId, UserId,
    },
    Client, HttpError,
};
use sqlx::query;
use twilight_model::id::{marker::UserMarker, Id};

/// Wrapped client used by this crate
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct VirtualClient {
    /// Inner client
    client: Client,
    /// Next sync token to use
    sync_token: Mutex<Option<String>>,
}

impl VirtualClient {
    /// Create a new virtualclient
    pub(super) fn new(client: Client) -> Self {
        Self {
            client,
            sync_token: Mutex::new(None),
        }
    }

    /// Perform a single sync
    pub(super) async fn sync_once(self: &Arc<Self>) -> Result<()> {
        let mut token = self.sync_token.lock().await;

        let mut sync_settings = SyncSettings::new().timeout(Duration::from_secs(0));
        if let Some(token) = token.as_ref() {
            sync_settings = sync_settings.token(token.clone());
        }

        let response = self.client.sync_once(sync_settings).await?;

        *token = Some(response.next_batch);
        Ok(())
    }

    /// Join a room by id
    pub(super) async fn join_room_by_id(self: &Arc<Self>, room_id: &RoomId) -> Result<Room> {
        // Make sure that we are up to date
        self.sync_once().await?;

        match self.get_room(room_id) {
            Some(Room::Joined(room)) => Ok(Room::Joined(room)),
            Some(Room::Invited(room)) => {
                room.accept_invitation().await?;
                self.sync_once().await?;
                self.get_room(room_id)
                    .ok_or_else(|| anyhow::anyhow!("Room not found"))
            }
            r => r.ok_or_else(|| anyhow::anyhow!("Room not found")),
        }
    }
}

impl Deref for VirtualClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

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
    pub async fn client(
        self: &Arc<Self>,
        user_id: Option<Id<UserMarker>>,
    ) -> Result<Arc<VirtualClient>> {
        match user_id {
            None => Ok(Arc::clone(&self.client)),
            Some(user_id) => {
                if let Some(client) = self.discord_clients.get(&user_id) {
                    Ok(Arc::clone(&*client))
                } else {
                    let username = format!("{}_discord_{user_id}", self.config.bridge.prefix);
                    self.try_register_user(&username).await?;
                    let user = Arc::new(VirtualClient::new(
                        self.appservice.virtual_user_client(&username).await?,
                    ));
                    self.discord_clients.insert(user_id, Arc::clone(&user));
                    Ok(user)
                }
            }
        }
    }

    /// Returns the room for a client
    ///
    /// # Errors
    /// This function will return an error if retrieving the room fails
    pub async fn matrix_room_for_client(
        self: &Arc<Self>,
        user_id: Option<Id<UserMarker>>,
        room_id: &RoomId,
    ) -> Result<Room> {
        self.client(user_id).await?.join_room_by_id(room_id).await
    }

    /// Unregisters a matrix user
    #[allow(clippy::panic)]
    pub(super) async fn unregister_user(self: &Arc<Self>, user: &UserId) -> Result<()> {
        query!(
            "DELETE FROM discord_tokens WHERE user_id = $1",
            user.as_str()
        )
        .execute(&*self.db)
        .await?;
        Ok(())
    }

    /// Registers a matrix user
    #[allow(clippy::panic)]
    pub(super) async fn register_user(
        self: &Arc<Self>,
        user: &UserId,
        room: &RoomId,
        token: &str,
    ) -> Result<()> {
        self.unregister_user(user).await?;
        query!(
            "INSERT INTO discord_tokens (user_id, token, management_room) VALUES ($1, $2, $3)",
            user.as_str(),
            token,
            room.as_str()
        )
        .execute(&*self.db)
        .await?;
        Ok(())
    }
}
