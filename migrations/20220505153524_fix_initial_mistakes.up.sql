ALTER TABLE statestore_stripped_members RENAME COLUMN user_id TO state_key;

-- The primary key should have been (room_id, receipt_type, user)
ALTER TABLE statestore_room_receipts DROP CONSTRAINT statestore_room_receipts_pkey;
ALTER TABLE statestore_room_receipts ADD PRIMARY KEY USING INDEX statestore_room_user_receipts;

-- (room_id, receipt_type, event) is not unique
DROP INDEX statestore_room_event_receipts;
CREATE INDEX statestore_room_event_receipts ON statestore_room_receipts (room_id, receipt_type, event_id);

DROP TABLE statestore_room_timeline; -- not used
