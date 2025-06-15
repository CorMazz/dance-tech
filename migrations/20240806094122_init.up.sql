-- Add up migration script here

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Enable pg_trgm extension
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Set similarity threshold (optional, adjust as needed)
SET pg_trgm.similarity_threshold = 0.3;

create table
    "users" (
        id uuid primary key default (uuid_generate_v4()),
        first_name varchar(100) not null,
        last_name varchar(100) not null,
        email varchar(255) not null unique,
        password varchar(100) not null,
        roles JSONB not null default '[]',
        created_at timestamp
        with
            time zone default now(),
            updated_at timestamp
        with
            time zone default now()
    );

CREATE INDEX users_email_idx ON users (email);

