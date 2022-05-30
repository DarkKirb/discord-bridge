//! App

use std::{str::FromStr, sync::Arc};

use crate::{Args, ConfigFile};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use matrix_sdk::{
    config::{StoreConfig, SyncSettings},
    event_handler::Ctx,
    room::Room,
    ruma::{
        events::room::member::{MembershipState, OriginalSyncRoomMemberEvent},
        ServerName, UserId,
    },
    Client, Session,
};
use matrix_sdk_appservice::{AppService, AppServiceRegistration};
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions, PgPool,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{debug, info, log::LevelFilter};

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
}

impl App {
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
        let session = Session {
            access_token: appservice.registration().as_token.clone(),
            user_id,
            device_id: discordbot_name.into(),
        };
        client.restore_login(session).await?;

        let (sender, mut receiver) = mpsc::unbounded_channel();

        let arc = Arc::new(Self {
            config: config.clone(),
            appservice,
            db,
            queue: sender,
            client,
        });

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
        arc.client.register_event_handler(
            |event: OriginalSyncRoomMemberEvent, room: Room, Ctx(this): Ctx<Arc<Self>>| async move {
                this.handle_room_member(room, event).await
            },
        ).await;

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
        event: OriginalSyncRoomMemberEvent,
    ) -> Result<()> {
        info!("Handling room member event: {:?}", event);
        if event.content.membership == MembershipState::Invite {
            let user_id = UserId::parse(event.state_key.as_str())?;
            if user_id
                != self
                    .client
                    .user_id()
                    .await
                    .ok_or_else(|| anyhow!("No user id"))?
            {
                return Ok(());
            }
            self.client.join_room_by_id(room.room_id()).await?;
        }
        Ok(())
    }
}
