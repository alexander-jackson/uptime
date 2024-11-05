CREATE TABLE origin (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	origin_uid UUID NOT NULL,
	uri TEXT NOT NULL,

	CONSTRAINT pk_origin PRIMARY KEY (id),
	CONSTRAINT uk_origin_origin_uid UNIQUE (origin_uid)
);

CREATE TABLE query (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	query_uid UUID NOT NULL,
	origin_id BIGINT NOT NULL,
	status SMALLINT NOT NULL,
	latency_millis BIGINT NOT NULL,
	queried_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_query PRIMARY KEY (id),
	CONSTRAINT uk_query_query_uid UNIQUE (query_uid),
	CONSTRAINT fk_query_origin_id FOREIGN KEY (origin_id) REFERENCES origin (id)
);

CREATE INDEX idx_query_queried_at_desc ON query (queried_at DESC);

CREATE TABLE query_failure_reason (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	name TEXT NOT NULL,

	CONSTRAINT pk_query_failure_reason PRIMARY KEY (id),
	CONSTRAINT uk_query_failure_reason_name UNIQUE (name)
);

INSERT INTO query_failure_reason (name)
VALUES
	('RequestTimeout'),
	('Redirection'),
	('BadRequest'),
	('ConnectionFailure'),
	('InvalidBody'),
	('Unknown');

CREATE TABLE query_failure (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	query_failure_uid UUID NOT NULL,
	origin_id BIGINT NOT NULL,
	failure_reason_id BIGINT NOT NULL,
	queried_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_query_failure PRIMARY KEY (id),
	CONSTRAINT uk_query_failure_query_failure_uid UNIQUE (query_failure_uid),
	CONSTRAINT fk_query_failure_origin_id FOREIGN KEY (origin_id) REFERENCES origin (id),
	CONSTRAINT fk_query_failure_query_failure_reason_id FOREIGN KEY (failure_reason_id) REFERENCES query_failure_reason (id)
);

CREATE INDEX idx_query_failure_origin_queried_at ON query_failure (origin_id, queried_at DESC);
