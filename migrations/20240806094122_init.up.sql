-- Add up migration script here

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Enable pg_trgm extension
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Set similarity threshold (optional, adjust as needed)
SET pg_trgm.similarity_threshold = 0.3;

CREATE TABLE
    "users" (
        id UUID PRIMARY KEY DEFAULT (uuid_generate_v4()),
        first_name VARCHAR(100) NOT NULL,
        last_name VARCHAR(100) NOT NULL,
        email VARCHAR(255) NOT NULL UNIQUE,
        password VARCHAR(100) NOT NULL,
        created_at TIMESTAMP
        WITH
            TIME ZONE DEFAULT NOW(),
            updated_at TIMESTAMP
        WITH
            TIME ZONE DEFAULT NOW()
    );

CREATE INDEX users_email_idx ON users (email);

