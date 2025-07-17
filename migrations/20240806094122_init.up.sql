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

CREATE INDEX users_email_trgm_idx
ON users
USING GIN (email gin_trgm_ops);

create table
    "graded_exams" (
        id uuid primary key,
        test_data JSONB not null,
        created_at timestamp
        with
            time zone default now()
    );

create table
    "exam_queue" (
        id serial primary key,
        user_id uuid not null references users(id) on delete cascade,
        test_index integer not null,
        inserted_at timestamp
        with
            time zone default now(),
        unique(user_id, test_index)
      );
