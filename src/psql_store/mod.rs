//! matrix-sdk store based on Postgres

use std::collections::BTreeSet;
use std::sync::Arc;

use color_eyre::Result;
use matrix_sdk::deserialized_responses::{MemberEvent, SyncRoomEvent};
use matrix_sdk::media::MediaRequest;
use matrix_sdk::ruma::events::presence::PresenceEvent;
use matrix_sdk::ruma::events::receipt::Receipt;
use matrix_sdk::ruma::events::room::member::{MembershipState, RoomMemberEventContent};
use matrix_sdk::ruma::events::{
    AnyGlobalAccountDataEvent, AnyRoomAccountDataEvent, AnyStrippedStateEvent, AnySyncStateEvent,
    GlobalAccountDataEventType, OriginalSyncStateEvent, RoomAccountDataEventType, StateEventType,
    StrippedStateEvent,
};
use matrix_sdk::ruma::receipt::ReceiptType;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::{EventId, MxcUri, OwnedEventId, OwnedUserId, RoomId, UserId};
use matrix_sdk::{async_trait, RoomInfo, StateChanges, StoreError};
use matrix_sdk_base::store::{BoxStream, Result as StateResult};
use matrix_sdk_base::StateStore;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{query, PgPool, Postgres, Transaction};

/// State store for postgresql databases
#[derive(Clone, Debug)]
pub struct PostgresStateStore {
    /// Postgresql database
    pool: Arc<PgPool>,
}

#[allow(clippy::panic)]
impl PostgresStateStore {
    /// Creates a new postgres state store
    #[must_use]
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Save the given filter id under the given name
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_filter(&self, filter_name: &str, filter_id: &str) -> Result<()> {
        query!(
            r#"
            INSERT INTO
                statestore_filters
                    (filter_name, filter_id)
            VALUES ($1, $2)
            ON CONFLICT (filter_name)
                DO UPDATE SET filter_id = EXCLUDED.filter_id
        "#,
            filter_name,
            filter_id
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    /// Saves a new sync token
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_sync_token(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        sync_token: impl AsRef<str> + Send,
    ) -> Result<()> {
        query!(
            r#"
            INSERT INTO
                statestore_misc (misc_key, misc_value)
            VALUES ($1, $2)
            ON CONFLICT (misc_key)
                DO UPDATE SET misc_value = EXCLUDED.misc_value
            "#,
            "sync_token",
            sync_token.as_ref()
        )
        .execute(txn)
        .await?;

        Ok(())
    }

    /// Mark a member as a specific state
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    #[allow(clippy::trait_duplication_in_bounds)]
    async fn set_member_room_status(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        state_key: impl AsRef<str> + Send,
        status: impl AsRef<str> + Send,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO
                    statestore_room_user_ids (room_id, user_id, user_status)
                VALUES ($1, $2, $3)
                ON CONFLICT (room_id, user_id)
                    DO UPDATE SET user_status = EXCLUDED.user_status
            "#,
            room.as_ref(),
            state_key.as_ref(),
            status.as_ref()
        )
        .execute(txn)
        .await?;

        Ok(())
    }

    /// Remove a member from a room
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn remove_member_room_status(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        state_key: impl AsRef<str> + Send,
    ) -> Result<()> {
        query!(
            r#"
            DELETE FROM statestore_room_user_ids
            WHERE
                (room_id = $1)
            AND (user_id = $2)
        "#,
            room.as_ref(),
            state_key.as_ref()
        )
        .execute(txn)
        .await?;

        Ok(())
    }

    /// Stores member synchronization info in the database
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_member(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        member: impl AsRef<str> + Send,
        event: &OriginalSyncStateEvent<RoomMemberEventContent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_members
                    (room_id, user_id, sync_content)
                VALUES
                    ($1, $2, $3)
                ON CONFLICT (room_id, user_id)
                    DO UPDATE SET sync_content = EXCLUDED.sync_content
            "#,
            room.as_ref(),
            member.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates a user profile
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_profile(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        user_id: impl AsRef<str> + Send,
        profile: &RoomMemberEventContent,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_profiles
                    (room_id, user_id, profile_data)
                VALUES
                    ($1, $2, $3)
                ON CONFLICT (room_id, user_id)
                    DO UPDATE SET profile_data = EXCLUDED.profile_data
            "#,
            room.as_ref(),
            user_id.as_ref(),
            Json(profile) as _
        )
        .execute(txn)
        .await?;

        Ok(())
    }

    /// Updates a display name
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_displayname(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        user_id: impl AsRef<str> + Send,
        displayname: impl AsRef<str> + Send,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_displaynames
                    (room_id, user_id, displayname)
                VALUES
                    ($1, $2, $3)
                ON CONFLICT (room_id, user_id)
                    DO UPDATE SET displayname = EXCLUDED.displayname
            "#,
            room.as_ref(),
            user_id.as_ref(),
            displayname.as_ref()
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates a your account data
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_account_data(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        event_type: impl AsRef<str> + Send,
        event: &Raw<AnyGlobalAccountDataEvent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_accountdata
                    (event_type, event_data)
                VALUES
                    ($1, $2)
                ON CONFLICT (event_type)
                    DO UPDATE SET event_data = EXCLUDED.event_data
            "#,
            event_type.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates a your account data for a specific room
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_room_account_data(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        to_string: impl AsRef<str> + Send,
        event: &Raw<AnyRoomAccountDataEvent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_room_account_data
                    (room_id, event_type, account_data)
                VALUES
                    ($1, $2, $3)
                ON CONFLICT (room_id, event_type)
                    DO UPDATE SET account_data = EXCLUDED.account_data
            "#,
            room.as_ref(),
            to_string.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates room info
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_room_info(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room_id: impl AsRef<str> + Send,
        room_info: &RoomInfo,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_stripped_room_infos
                    (room_id, room_info)
                VALUES
                    ($1, $2)
                ON CONFLICT (room_id)
                    DO UPDATE SET room_info = EXCLUDED.room_info
            "#,
            room_id.as_ref(),
            Json(room_info) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates User presence
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_presence(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        user_id: impl AsRef<str> + Send,
        event: &Raw<PresenceEvent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_presence
                    (user_id, presence_event)
                VALUES
                    ($1, $2)
                ON CONFLICT (user_id)
                    DO UPDATE SET presence_event = EXCLUDED.presence_event
            "#,
            user_id.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates Stripped room info
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn update_stripped_room_info(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room_id: impl AsRef<str> + Send,
        info: &RoomInfo,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_stripped_room_infos
                    (room_id, room_info)
                VALUES
                    ($1, $2)
                ON CONFLICT (room_id)
                    DO UPDATE SET room_info = EXCLUDED.room_info
            "#,
            room_id.as_ref(),
            Json(info) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates Stripped member info
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_stripped_member(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        state_key: impl AsRef<str> + Send,
        event: &StrippedStateEvent<RoomMemberEventContent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_stripped_members
                    (room_id, state_key, member_event)
                VALUES
                    ($1, $2, $3)
                ON CONFLICT (room_id, state_key)
                    DO UPDATE SET member_event = EXCLUDED.member_event
            "#,
            room.as_ref(),
            state_key.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates Stripped event state
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_stripped_state(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        event_type: impl AsRef<str> + Send,
        state_key: impl AsRef<str> + Send,
        event: &Raw<AnyStrippedStateEvent>,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_stripped_room_state
                    (room_id, event_type, state_key, state_event)
                VALUES
                    ($1, $2, $3, $4)
                ON CONFLICT (room_id, event_type, state_key)
                    DO UPDATE SET state_event = EXCLUDED.state_event
            "#,
            room.as_ref(),
            event_type.as_ref(),
            state_key.as_ref(),
            Json(event) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Updates Room receipt state
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_room_receipts(
        &self,
        txn: &mut Transaction<'_, Postgres>,
        room: impl AsRef<str> + Send,
        event_id: impl AsRef<str> + Send,
        receipt_type: impl AsRef<str> + Send,
        user_id: impl AsRef<str> + Send,
        receipt: &Receipt,
    ) -> Result<()> {
        query!(
            r#"
                INSERT INTO statestore_room_receipts
                    (room_id, receipt_type, user_id, event_id, receipt)
                VALUES
                    ($1, $2, $3, $4, $5)
                ON CONFLICT (room_id, receipt_type, user_id)
                    DO UPDATE SET event_id = EXCLUDED.event_id, receipt = EXCLUDED.receipt
            "#,
            room.as_ref(),
            receipt_type.as_ref(),
            user_id.as_ref(),
            event_id.as_ref(),
            Json(receipt) as _
        )
        .execute(txn)
        .await?;
        Ok(())
    }

    /// Save state changes to database
    ///
    /// # Errors
    /// This function will return an error if updating the database fails
    async fn save_changes(&self, changes: &StateChanges) -> Result<()> {
        let mut txn = self.pool.begin().await?;

        if let Some(s) = &changes.sync_token {
            self.save_sync_token(&mut txn, s).await?;
        }

        for (room, events) in &changes.members {
            for event in events.values() {
                match event.content.membership {
                    MembershipState::Join => {
                        self.set_member_room_status(&mut txn, room, &event.state_key, "joined")
                            .await?;
                    }
                    MembershipState::Invite => {
                        self.set_member_room_status(&mut txn, room, &event.state_key, "invited")
                            .await?;
                    }
                    _ => {
                        self.remove_member_room_status(&mut txn, room, &event.state_key)
                            .await?;
                    }
                }
                self.save_member(&mut txn, room, &event.state_key, event)
                    .await?;
            }
        }

        for (room, users) in &changes.profiles {
            for (user_id, profile) in users {
                self.update_profile(&mut txn, room, user_id, profile)
                    .await?;
            }
        }

        for (room, map) in &changes.ambiguity_maps {
            for (display_name, user_ids) in map {
                for user_id in user_ids {
                    self.update_displayname(&mut txn, room, user_id, display_name)
                        .await?;
                }
            }
        }

        for (event_type, event) in &changes.account_data {
            self.update_account_data(&mut txn, event_type.to_string(), event)
                .await?;
        }

        for (room, events) in &changes.room_account_data {
            for (event_type, event) in events {
                self.update_room_account_data(&mut txn, room, event_type.to_string(), event)
                    .await?;
            }
        }

        for (room_id, room_info) in &changes.room_infos {
            self.update_room_info(&mut txn, room_id, room_info).await?;
        }

        for (sender, event) in &changes.presence {
            self.update_presence(&mut txn, sender, event).await?;
        }

        for (room_id, info) in &changes.stripped_room_infos {
            self.update_stripped_room_info(&mut txn, room_id, info)
                .await?;
        }

        for (room, events) in &changes.stripped_members {
            for event in events.values() {
                self.save_stripped_member(&mut txn, room, &event.state_key, event)
                    .await?;
            }
        }

        for (room, event_types) in &changes.stripped_state {
            for (event_type, events) in event_types {
                for (state_key, event) in events {
                    self.save_stripped_state(
                        &mut txn,
                        room,
                        event_type.to_string(),
                        state_key,
                        event,
                    )
                    .await?;
                }
            }
        }

        for (room, content) in &changes.receipts {
            for (event_id, receipts) in &content.0 {
                for (receipt_type, receipts) in receipts {
                    for (user_id, receipt) in receipts {
                        self.save_room_receipts(
                            &mut txn,
                            room,
                            event_id,
                            receipt_type,
                            user_id,
                            receipt,
                        )
                        .await?;
                    }
                }
            }
        }

        txn.commit().await?;
        Ok(())
    }
}

impl From<Arc<PgPool>> for PostgresStateStore {
    fn from(pool: Arc<PgPool>) -> Self {
        Self::new(pool)
    }
}

#[async_trait]
impl StateStore for PostgresStateStore {
    /// Save the given filter id under the given name.
    ///
    /// # Arguments
    ///
    /// * `filter_name` - The name that should be used to store the filter id.
    ///
    /// * `filter_id` - The filter id that should be stored in the state store.
    #[allow(clippy::panic)]
    async fn save_filter(&self, filter_name: &str, filter_id: &str) -> StateResult<()> {
        Ok(self
            .save_filter(filter_name, filter_id)
            .await
            .map_err(|e| StoreError::Backend(e.into()))?)
    }

    /// Save the set of state changes in the store.
    async fn save_changes(&self, changes: &StateChanges) -> StateResult<()> {
        Ok(self
            .save_changes(changes)
            .await
            .map_err(|e| StoreError::Backend(e.into()))?)
    }

    /// Get the filter id that was stored under the given filter name.
    ///
    /// # Arguments
    ///
    /// * `filter_name` - The name that was used to store the filter id.
    async fn get_filter(&self, filter_name: &str) -> StateResult<Option<String>> {
        todo!();
    }

    /// Get the last stored sync token.
    async fn get_sync_token(&self) -> StateResult<Option<String>> {
        todo!();
    }

    /// Get the stored presence event for the given user.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The id of the user for which we wish to fetch the presence
    /// event for.
    async fn get_presence_event(
        &self,
        user_id: &UserId,
    ) -> StateResult<Option<Raw<PresenceEvent>>> {
        todo!();
    }

    /// Get a state event out of the state store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room the state event was received for.
    ///
    /// * `event_type` - The event type of the state event.
    async fn get_state_event(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_key: &str,
    ) -> StateResult<Option<Raw<AnySyncStateEvent>>> {
        todo!();
    }

    /// Get a list of state events for a given room and `StateEventType`.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room to find events for.
    ///
    /// * `event_type` - The event type.
    async fn get_state_events(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
    ) -> StateResult<Vec<Raw<AnySyncStateEvent>>> {
        todo!();
    }

    /// Get the current profile for the given user in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The room id the profile is used in.
    ///
    /// * `user_id` - The id of the user the profile belongs to.
    async fn get_profile(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
    ) -> StateResult<Option<RoomMemberEventContent>> {
        todo!();
    }

    /// Get a raw `MemberEvent` for the given state key in the given room id.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The room id the member event belongs to.
    ///
    /// * `state_key` - The user id that the member event defines the state for.
    async fn get_member_event(
        &self,
        room_id: &RoomId,
        state_key: &UserId,
    ) -> StateResult<Option<MemberEvent>> {
        todo!();
    }

    /// Get all the user ids of members for a given room.
    async fn get_user_ids(&self, room_id: &RoomId) -> StateResult<Vec<OwnedUserId>> {
        todo!();
    }

    /// Get all the user ids of members that are in the invited state for a
    /// given room.
    async fn get_invited_user_ids(&self, room_id: &RoomId) -> StateResult<Vec<OwnedUserId>> {
        todo!();
    }

    /// Get all the user ids of members that are in the joined state for a
    /// given room.
    async fn get_joined_user_ids(&self, room_id: &RoomId) -> StateResult<Vec<OwnedUserId>> {
        todo!();
    }

    /// Get all the pure `RoomInfo`s the store knows about.
    async fn get_room_infos(&self) -> StateResult<Vec<RoomInfo>> {
        todo!();
    }

    /// Get all the pure `RoomInfo`s the store knows about.
    async fn get_stripped_room_infos(&self) -> StateResult<Vec<RoomInfo>> {
        todo!();
    }

    /// Get all the users that use the given display name in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the display name users should
    /// be fetched for.
    ///
    /// * `display_name` - The display name that the users use.
    async fn get_users_with_display_name(
        &self,
        room_id: &RoomId,
        display_name: &str,
    ) -> StateResult<BTreeSet<OwnedUserId>> {
        todo!();
    }

    /// Get an event out of the account data store.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The event type of the account data event.
    async fn get_account_data_event(
        &self,
        event_type: GlobalAccountDataEventType,
    ) -> StateResult<Option<Raw<AnyGlobalAccountDataEvent>>> {
        todo!();
    }

    /// Get an event out of the room account data store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the room account data event
    ///   should
    /// be fetched.
    ///
    /// * `event_type` - The event type of the room account data event.
    async fn get_room_account_data_event(
        &self,
        room_id: &RoomId,
        event_type: RoomAccountDataEventType,
    ) -> StateResult<Option<Raw<AnyRoomAccountDataEvent>>> {
        todo!();
    }

    /// Get an event out of the user room receipt store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the receipt should be
    ///   fetched.
    ///
    /// * `receipt_type` - The type of the receipt.
    ///
    /// * `user_id` - The id of the user for who the receipt should be fetched.
    async fn get_user_room_receipt_event(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        user_id: &UserId,
    ) -> StateResult<Option<(OwnedEventId, Receipt)>> {
        todo!();
    }

    /// Get events out of the event room receipt store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the receipts should be
    ///   fetched.
    ///
    /// * `receipt_type` - The type of the receipts.
    ///
    /// * `event_id` - The id of the event for which the receipts should be
    ///   fetched.
    async fn get_event_room_receipt_events(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        event_id: &EventId,
    ) -> StateResult<Vec<(OwnedUserId, Receipt)>> {
        todo!();
    }

    /// Get arbitrary data from the custom store
    ///
    /// # Arguments
    ///
    /// * `key` - The key to fetch data for
    async fn get_custom_value(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        todo!();
    }

    /// Put arbitrary data into the custom store
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert data into
    ///
    /// * `value` - The value to insert
    async fn set_custom_value(&self, key: &[u8], value: Vec<u8>) -> StateResult<Option<Vec<u8>>> {
        todo!();
    }

    /// Add a media file's content in the media store.
    ///
    /// # Arguments
    ///
    /// * `request` - The `MediaRequest` of the file.
    ///
    /// * `content` - The content of the file.
    async fn add_media_content(&self, request: &MediaRequest, content: Vec<u8>) -> StateResult<()> {
        todo!();
    }

    /// Get a media file's content out of the media store.
    ///
    /// # Arguments
    ///
    /// * `request` - The `MediaRequest` of the file.
    async fn get_media_content(&self, request: &MediaRequest) -> StateResult<Option<Vec<u8>>> {
        todo!();
    }

    /// Removes a media file's content from the media store.
    ///
    /// # Arguments
    ///
    /// * `request` - The `MediaRequest` of the file.
    async fn remove_media_content(&self, request: &MediaRequest) -> StateResult<()> {
        todo!();
    }

    /// Removes all the media files' content associated to an `MxcUri` from the
    /// media store.
    ///
    /// # Arguments
    ///
    /// * `uri` - The `MxcUri` of the media files.
    async fn remove_media_content_for_uri(&self, uri: &MxcUri) -> StateResult<()> {
        todo!();
    }

    /// Removes a room and all elements associated from the state store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The `RoomId` of the room to delete.
    async fn remove_room(&self, room_id: &RoomId) -> StateResult<()> {
        todo!();
    }
}
