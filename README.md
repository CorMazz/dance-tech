# Dancetech

## Note from Cory

This application is containerized, and was also developed in a dev container. Everything within the .devcontainer folder defines the configuration
for the dev container, and the other docker files out here define how to create the production containers. 

## Developer Notes

### Getting Started

#### Development Environment

The dev environment for this project is a dev container. The dev container pins the `rust` version. To update that, configure the `.devcontainer/devcontainer.json` file.

#### Environment Variables

The dev container will attempt to load the environment variables from a `.env_prod` file, as defined in the `.devcontainer/Dockerfile.`. Create and populate that `.env_prod` file, using the `generate_keys.sh` script to generate two sets of keys for the access and refresh tokens.

#### Updating Tailwind

The `tailwindcss` executable is too large for GitHub. The following commands will download and move the `tailwindcss` executable to where it should be for this project. Update the version if you want.

```bash
curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/download/v4.1.5/tailwindcss-linux-x64
chmod +x tailwindcss-linux-x64 
mv tailwindcss-linux-x64 tailwind/tailwindcss
```

### Development Workflow

Tailwind must be rebuilt everytime you make changes to the html classes. That can be done with the `tailwind/tailwindcss` executable.

`./tailwind/tailwindcss -i ./static/css/input.css -o ./static/css/output.css -c ./tailwind/tailwind.config.js`

If you want to automagically recompile your Rust executable and rebuild your css everytime you save a file, you can run this command.

`cargo watch -s './tailwind/tailwindcss -i ./static/css/input.css -o ./static/css/output.css -c ./tailwind/tailwind.config.js && cargo sqlx prepare && cargo run' --ignore *css* --ignore .sqlx --ignore main.rs --why`

For some reason the `cargo sqlx prepare` command changes the permissions on the `main.rs` file, which was causing cargo watch to fire repeatedly. That is why we added `--ignore main.rs`. 
The `cargo sqlx prepare` command allows sqlx to compile even when the database is offline. 

### Launching Production Containers

To build the two environments locally:

`ENV_FILE=.env_prod DOCKER_PORT_MAPPING=7000 SERVER_PORT=8000 PG_ADMIN_DOCKER_PORT_MAPPING=7001 docker-compose -p dancexam-prod up --build`

`ENV_FILE=.env_demo DOCKER_PORT_MAPPING=7002 SERVER_PORT=8000 PG_ADMIN_DOCKER_PORT_MAPPING=7003 docker-compose -p dancexam-demo up --build`

### Updating Flowbite + HTMX

#### Flowbite

Run the following commands to install Flowbite. Update the version first. [Why didn't I use CDNs?](https://blog.wesleyac.com/posts/why-not-javascript-cdn)

```bash
curl -LO https://cdn.jsdelivr.net/npm/flowbite@3.1.2/dist/flowbite.min.css
mv flowbite.min.css static/css/
curl -LO https://cdn.jsdelivr.net/npm/flowbite@3.1.2/dist/flowbite.min.js
mv flowbite.min.js static/js
```

#### HTMX

Run the following commands to install HTMX. Update the version first. [Why didn't I use CDNs?](https://blog.wesleyac.com/posts/why-not-javascript-cdn)

```bash
curl -LO https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js
mv htmx.min.js static/js/
```

### Odd Errors

#### `static/` Not Served Properly (404 Errors on Assets)

Ensure you are launching the server from the root of this repository. File paths are relative to the directory that the server is launched from.