CREATE TABLE events (
                               id character varying(255) NOT NULL,
                               block_height integer,
                               module character varying(255),
                               event character varying(255)
);

CREATE TABLE extrinsics (
                                   id character varying(255) NOT NULL,
                                   tx_hash character varying(255),
                                   block_height integer,
                                   module character varying(255),
                                   call character varying(255),
                                   success boolean,
                                   is_signed boolean
);


CREATE TABLE spec_versions (
                                      id character varying(255) NOT NULL,
                                      block_height integer
);



ALTER TABLE ONLY events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);



ALTER TABLE ONLY extrinsics
    ADD CONSTRAINT extrinsics_pkey PRIMARY KEY (id);


ALTER TABLE ONLY spec_versions
    ADD CONSTRAINT spec_versions_pkey PRIMARY KEY (id);


