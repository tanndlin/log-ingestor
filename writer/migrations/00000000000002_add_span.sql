ALTER TABLE traces ADD COLUMN span_id BIGINT;
CREATE INDEX traces_span_id_idx ON traces (span_id);