CREATE TABLE discord_tokens(
  user_id TEXT PRIMARY KEY NOT NULL,
  token TEXT NOT NULL,
  management_room TEXT NOT NULL
);
