set dotenv-load := true

default:
    @just --list

bootstrap:
    cargo fetch
    npm --prefix frontend/web install

dev-api:
    cargo run -p multicloud-api

dev-web:
    npm --prefix frontend/web run dev

worker:
    cargo run -p multicloud-worker

scheduler:
    cargo run -p multicloud-scheduler

infra-up:
    podman compose up -d

infra-down:
    podman compose down

migrate *args:
    cargo run -p multicloud-migrations -- {{args}}

admin-init *args:
    cargo run -p multicloud-admin -- init {{args}}

recover-access *args:
    cargo run -p multicloud-admin -- recover-access {{args}}

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace
    npm --prefix frontend/web run check

check: fmt lint test
    npm --prefix frontend/web run build
