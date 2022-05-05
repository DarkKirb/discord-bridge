CREATE TABLE statestore_misc (
  misc_key TEXT PRIMARY KEY NOT NULL,
  misc_value TEXT NOT NULL
);

CREATE TABLE statestore_filters (
  filter_name TEXT PRIMARY KEY NOT NULL,
  filter_id TEXT NOT NULL
);

CREATE TABLE statestore_accountdata (
  event_type TEXT PRIMARY KEY NOT NULL,
  event_data JSONB NOT NULL
);

CREATE TABLE statestore_members (
  room_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  sync_content JSONB NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE statestore_profiles (
  room_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  profile_data JSONB NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE statestore_displaynames (
  room_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  displayname TEXT NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE statestore_room_user_ids (
  room_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  user_status TEXT NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE statestore_room_info (
  room_id TEXT PRIMARY KEY NOT NULL,
  room_info JSONB NOT NULL
);

CREATE TABLE statestore_room_state (
  room_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  state_key TEXT NOT NULL,
  state_event JSONB NOT NULL,
  PRIMARY KEY (room_id, event_type, state_key)
);

CREATE TABLE statestore_room_account_data (
  room_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  account_data JSONB NOT NULL,
  PRIMARY KEY (room_id, event_type)
);

CREATE TABLE statestore_stripped_room_infos (
  room_id TEXT PRIMARY KEY NOT NULL,
  room_info JSONB NOT NULL
);

CREATE TABLE statestore_stripped_room_state (
  room_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  state_key TEXT NOT NULL,
  state_event JSONB NOT NULL,
  PRIMARY KEY (room_id, event_type, state_key)
);

CREATE TABLE statestore_stripped_members (
  room_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  member_event JSONB NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE statestore_presence (
  user_id TEXT PRIMARY KEY NOT NULL,
  presence_event JSONB NOT NULL
);

CREATE TABLE statestore_room_receipts (
  room_id TEXT NOT NULL,
  receipt_type TEXT NOT NULL,
  user_id TEXT NOT NULL,
  event_id TEXT NOT NULL,
  receipt JSONB NOT NULL,
  PRIMARY KEY (room_id, receipt_type, user_id, event_id)
);

CREATE UNIQUE INDEX statestore_room_user_receipts ON statestore_room_receipts (room_id, receipt_type, user_id);
CREATE UNIQUE INDEX statestore_room_event_receipts ON statestore_room_receipts (room_id, receipt_type, event_id);

CREATE TABLE statestore_custom (
  custom_key BYTEA PRIMARY KEY NOT NULL,
  custom_value BYTEA NOT NULL
);

CREATE TABLE statestore_room_timeline (
  room_id TEXT PRIMARY KEY NOT NULL,
  timeline JSONB NOT NULL
);
