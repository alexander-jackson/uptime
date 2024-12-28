CREATE TABLE notification (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	notification_uid UUID NOT NULL,
	origin_id BIGINT NOT NULL,
	topic TEXT NOT NULL,
	subject TEXT NOT NULL,
	message TEXT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_notification PRIMARY KEY (id),
	CONSTRAINT uk_notification_notification_uid UNIQUE (notification_uid),
	CONSTRAINT fk_notification_origin_id FOREIGN KEY (origin_id) REFERENCES origin (id)
);
