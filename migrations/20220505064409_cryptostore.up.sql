CREATE TABLE cryptostore_misc(
  misc_key TEXT PRIMARY KEY NOT NULL,
  misc_val JSONB NOT NULL
);

CREATE TABLE cryptostore_sessions(
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  sender_key BYTEA NOT NULL,
  session_data JSONB NOT NULL
);

CREATE TABLE cryptostore_group_sessions(
  room_id TEXT NOT NULL,
  sender_key BYTEA NOT NULL,
  session_id TEXT NOT NULL,
  group_session JSONB NOT NULL,
  PRIMARY KEY (room_id, sender_key, session_id)
);

CREATE TABLE cryptostore_tracked_users(
  user_id TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE cryptostore_users_for_key_query (
  user_id TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE cryptostore_olm_hashes (
  sender_key TEXT NOT NULL,
  olm_hash TEXT NOT NULL,
  PRIMARY KEY (sender_key, olm_hash)
);

CREATE TABLE cryptostore_devices (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  device_info JSONB NOT NULL,
  PRIMARY KEY (user_id, device_id)
);

CREATE TABLE cryptostore_identities (
  user_id TEXT PRIMARY KEY NOT NULL,
  user_identity JSONB NOT NULL
);

CREATE TABLE cryptostore_outgoing_key_requests (
  tx_id TEXT PRIMARY KEY NOT NULL,
  gossip JSONB NOT NULL
);

CREATE TABLE cryptostore_key_requests_by_info (
  info TEXT PRIMARY KEY NOT NULL,
  tx_id TEXT NOT NULL
);
