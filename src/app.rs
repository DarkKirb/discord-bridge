//! App

use std::{str::FromStr, sync::Arc, time::Duration};

use crate::{Args, ConfigFile};
use anyhow::Result;
use matrix_sdk::{
    config::{RequestConfig, StoreConfig, SyncSettings},
    deserialized_responses::EncryptionInfo,
    event_handler::Ctx,
    room::Room,
    ruma::{
        api::client::{
            session::login::{
                self,
                v3::{ApplicationService, LoginInfo},
            },
            uiaa::UserIdentifier,
        },
        events::room::{member::StrippedRoomMemberEvent, message::SyncRoomMessageEvent},
        DeviceId, OwnedDeviceId, OwnedUserId, ServerName, UserId,
    },
    Client, Session,
};
use matrix_sdk_appservice::{AppService, AppServiceRegistration};
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions, PgPool,
};
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    time::sleep,
};
use tracing::{debug, error, info, log::LevelFilter, warn};

/// Queue events that need to be handled
#[derive(Clone, Debug)]
enum QueueEvent {
    Close,
}

/// Application entrypoint
#[derive(Debug)]
pub struct App {
    /// The configuration file used
    config: ConfigFile,
    /// The appservice
    appservice: AppService,
    /// Database
    db: Arc<PgPool>,
    /// Event queue
    queue: UnboundedSender<QueueEvent>,
    /// discordbot client
    client: Client,
    /// discordbot user id
    user_id: OwnedUserId,
}

impl App {
    /// Returns the device id or creates a new one
    async fn device_id(self: &Arc<Self>) -> Result<OwnedDeviceId> {
        let device_id = self.client.store().get_custom_value(b"device_id").await?;
        if let Some(device_id) = device_id {
            let device_id = String::from_utf8(device_id)?;
            Ok(OwnedDeviceId::try_from(device_id)?)
        } else {
            let device_id = DeviceId::new();
            self.client
                .store()
                .set_custom_value(b"device_id", device_id.as_bytes().to_vec())
                .await?;
            Ok(device_id)
        }
    }
    /// Returns a cached session
    async fn client_session(self: &Arc<Self>) -> Result<Session> {
        let session = self.client.store().get_custom_value(b"session").await?;
        if let Some(session) = session {
            let session = serde_json::from_slice(&session)?;
            Ok(session)
        } else {
            let login_info = LoginInfo::ApplicationService(ApplicationService::new(
                UserIdentifier::UserIdOrLocalpart(self.user_id.as_str()),
            ));
            let mut request = login::v3::Request::new(login_info);
            let device_id = self.device_id().await?;
            request.device_id = Some(device_id.as_ref());
            request.initial_device_display_name = Some("discordbot");
            let request = self
                .appservice
                .get_cached_client(None)?
                .send(request, Some(RequestConfig::default().force_auth()))
                .await?;
            let session = Session {
                access_token: request.access_token,
                user_id: request.user_id,
                device_id: request.device_id,
            };
            let encoded_session = serde_json::to_vec(&session)?;
            self.client
                .store()
                .set_custom_value(b"session", encoded_session)
                .await?;
            Ok(session)
        }
    }
    /// Retrieve connection options from a config file
    fn get_connect_options(config: &ConfigFile) -> PgConnectOptions {
        let mut conn_opt = PgConnectOptions::new();

        if let Some(ref host) = config.bridge.db.host {
            conn_opt = conn_opt.host(host);
        }
        if let Some(port) = config.bridge.db.port {
            conn_opt = conn_opt.port(port);
        }
        if let Some(ref socket) = config.bridge.db.socket {
            conn_opt = conn_opt.socket(socket);
        }
        if let Some(ref user) = config.bridge.db.user {
            conn_opt = conn_opt.username(user);
        }
        if let Some(ref password) = config.bridge.db.password {
            conn_opt = conn_opt.password(password);
        }
        if let Some(ref database) = config.bridge.db.database {
            conn_opt = conn_opt.database(database);
        }
        if let Some(sslmode) = config
            .bridge
            .db
            .sslmode
            .as_ref()
            .and_then(|v| PgSslMode::from_str(v).ok())
        {
            conn_opt = conn_opt.ssl_mode(sslmode);
        }
        if let Some(ref sslrootcert) = config.bridge.db.sslrootcert {
            conn_opt = conn_opt.ssl_root_cert(sslrootcert);
        }
        if let Some(statement_cache_capacity) = config.bridge.db.statement_cache_capacity {
            conn_opt = conn_opt.statement_cache_capacity(statement_cache_capacity);
        }
        if let Some(ref application_name) = config.bridge.db.application_name {
            conn_opt = conn_opt.application_name(application_name);
        }
        if let Some(extra_float_digits) = config.bridge.db.extra_float_digits {
            conn_opt = conn_opt.extra_float_digits(Some(extra_float_digits));
        }
        conn_opt = conn_opt.options(config.bridge.db.options.clone());
        conn_opt.log_statements(LevelFilter::Debug);
        conn_opt
    }

    /// Runs the actual server
    ///
    /// # Errors
    /// This function will return an error if reading registration information fails
    #[tracing::instrument]
    pub async fn new(config: &ConfigFile, args: &Args) -> Result<Arc<Self>> {
        debug!("Reading registration data");
        let registration = AppServiceRegistration::try_from_yaml_file(&args.registration)?;

        debug!("Connecting to database");
        let db = Arc::new(PgPool::connect_with(Self::get_connect_options(config)).await?);

        debug!("Opening the statestore");
        let statestore = matrix_sdk_sql::StateStore::new(&db).await?;
        let mut statestore2 = matrix_sdk_sql::StateStore::new(&db).await?;
        statestore2.unlock().await?;
        let store_config = StoreConfig::new()
            .state_store(Box::new(statestore))
            .crypto_store(Box::new(statestore2));
        let client_builder = Client::builder()
            .homeserver_url(&config.homeserver.address)
            .store_config(store_config)
            .appservice_mode()
            .assert_identity();

        debug!("Creating appservice instance");
        let appservice = AppService::new(
            config.homeserver.address.as_str(),
            config.homeserver.domain.clone(),
            registration,
        )
        .await?;

        // register the discordbot
        let discordbot_name = format!("{}_discordbot", config.bridge.prefix);
        appservice
            .register_virtual_user(&discordbot_name)
            .await
            .ok();

        let user_id = UserId::parse_with_server_name(
            discordbot_name.clone(),
            <&ServerName>::try_from(config.homeserver.domain.as_str())?,
        )?;

        let client = client_builder.build().await?;

        let (sender, mut receiver) = mpsc::unbounded_channel();

        let arc = Arc::new(Self {
            config: config.clone(),
            appservice,
            db,
            queue: sender,
            client,
            user_id,
        });

        arc.client
            .restore_login(arc.client_session().await?)
            .await?;

        let arc2 = Arc::clone(&arc);
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let arc = Arc::clone(&arc2);
                match event {
                    QueueEvent::Close => {
                        debug!("Closing queue");
                        receiver.close();
                    }
                }
                let err = match tokio::spawn(async move { arc.handle_event(event).await }).await {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => e,
                    Err(e) => e.into(),
                };
                sentry::integrations::anyhow::capture_anyhow(&err);
                eprintln!("{:?}", err);
            }
        });

        arc.client.register_event_handler_context(Arc::clone(&arc));

        // Start registering handlers
        arc.client
            .register_event_handler(
                |event: StrippedRoomMemberEvent, room: Room, Ctx(this): Ctx<Arc<Self>>| async move {
                    this.handle_room_member(room, event).await
                },
            )
            .await
            .register_event_handler(
                |event: SyncRoomMessageEvent,
                 room: Room,
                 Ctx(this): Ctx<Arc<Self>>,
                 encryption_info: Option<EncryptionInfo>| async move {
                    println!(
                        "message {:?} in {:?} with encryption info {:?}",
                        event, room, encryption_info
                    );
                },
            )
            .await;
        Ok(arc)
    }

    /// Internal queue event handler
    async fn handle_event(self: &Arc<Self>, event: QueueEvent) -> Result<()> {
        match event {
            QueueEvent::Close => {}
        }
        Ok(())
    }

    /// Run the application
    ///
    /// # Errors
    /// This function will return an error if starting the application fails
    pub async fn run(self: &Arc<Self>) -> Result<()> {
        self.client.sync(SyncSettings::default()).await;
        Ok(())
    }

    /// Handle [`OriginalSyncRoomMemberEvent`]
    #[tracing::instrument]
    async fn handle_room_member(
        self: &Arc<Self>,
        room: Room,
        room_member: StrippedRoomMemberEvent,
    ) -> Result<()> {
        info!(
            "Handling room member event: {:?} in {:?}",
            room_member, room
        );
        if room_member.state_key != self.user_id {
            return Ok(());
        }
        if let Room::Invited(room) = room {
            info!("Autojoining room {}", room.room_id());
            let mut delay = 2;

            while let Err(err) = room.accept_invitation().await {
                // retry autojoin due to synapse sending invites, before the
                // invited user can join for more information see
                // https://github.com/matrix-org/synapse/issues/4345
                warn!(
                    "Failed to join room {} ({:?}), retrying in {}s",
                    room.room_id(),
                    err,
                    delay
                );

                sleep(Duration::from_secs(delay)).await;
                delay *= 2;

                if delay > 8 {
                    error!("Can't join room {} ({:?})", room.room_id(), err);
                    break;
                }
            }
            info!("Successfully joined room {}", room.room_id());
        }
        Ok(())
    }
}
