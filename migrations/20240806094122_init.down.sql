-- Add down migration script here

DROP TABLE IF EXISTS graded_exams;
DROP TABLE IF EXISTS exam_queue;
DROP TABLE IF EXISTS users;
DROP EXTENSION IF EXISTS pg_trgm;
DROP EXTENSION IF EXISTS "uuid-ossp";
