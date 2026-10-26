-- Gives every row a stable tiebreaker so the querier can keyset-paginate on
-- (timestamp, id) without skipping rows that share a timestamp.
ALTER TABLE traces ADD COLUMN id BIGSERIAL;
CREATE INDEX traces_timestamp_id_idx ON traces (timestamp, id);
