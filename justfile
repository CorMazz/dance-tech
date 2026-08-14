# Loads .env_prod by default (same file as the dev container).
# Override: just --dotenv-filename .env_demo preview
set dotenv-load
set dotenv-filename := ".env_prod"

# List recipes.
default:
    @just --list

# Compile. Does not start Postgres/Redis or the server.
build:
    cargo build

# Rebuild CSS, then watch templates/CSS and restart `cargo run`.
preview:
    bacon

# One-shot server. No file watching.
run:
    cargo run

# Rebuild Tailwind only.
css:
    ./tailwind/tailwindcss -i ./static/css/input.css -o ./static/css/output.css -c ./tailwind/tailwind.config.js
