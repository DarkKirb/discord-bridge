ALTER TABLE statestore_stripped_members RENAME COLUMN state_key TO user_id;

CREATE UNIQUE INDEX statestore_room_receipts_pkey ON statestore_room_receipts (room_id, receipt_type, user_id, event_id);
ALTER TABLE statestore_room_receipts DROP CONSTRAINT statestore_room_user_receipts;
ALTER TABLE statestore_room_receipts ADD PRIMARY KEY USING INDEX statestore_room_receipts_pkey;
CREATE UNIQUE INDEX statestore_room_user_receipts ON statestore_room_receipts (room_id, receipt_type, user_id);

CREATE TABLE statestore_room_timeline (
  room_id TEXT PRIMARY KEY NOT NULL,
  timeline JSONB NOT NULL
);

DROP INDEX statestore_room_event_receipts;
CREATE UNIQUE INDEX statestore_room_event_receipts ON statestore_room_receipts (room_id, receipt_type, event_id);
