CREATE INDEX idx_notification_origin_id_created_at
ON notification (origin_id, created_at DESC);
