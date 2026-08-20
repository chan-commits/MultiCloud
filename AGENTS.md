# Repository Guidelines

## Project Structure & Module Organization

MultiCloud is a Rust workspace with an Axum API and background services. `apps/multicloud` produces the single deployable `multicloud` binary; runtime modules live in the neighboring `apps/*` library crates, and reusable domain/infrastructure modules live in `crates/`. Database migrations are in `migrations/`. The Svelte 5/Vite/Tailwind frontend is under `frontend/web/src`, with UI components in `components/` and browser/API helpers in `lib/`. Architecture notes belong in `doc/`.

## Build, Test, and Development Commands

Use `just` recipes from the repository root:

- `just bootstrap` fetches crates and installs frontend dependencies.
- `just infra-up` / `just infra-down` starts or stops local Podman services.
- `just run` starts the combined API/Worker/Scheduler process; `just dev-web` runs the frontend. `just worker` and `just scheduler` invoke isolated subcommands when needed.
- `just migrate ...` runs database migrations; `just admin-init ...` initializes the platform admin CLI.
- `just fmt` formats Rust and frontend code.
- `just check` runs Clippy, workspace tests, Svelte checks, formatting checks, and the production web build.

For focused frontend work, use `npm --prefix frontend/web run check`, `format`, or `build`. Preserve `package-lock.json` when dependencies change.

## Coding Style & Naming Conventions

Rust uses `rustfmt` defaults, four-space indentation, snake_case modules/functions, and PascalCase types. Keep domain logic in its owning crate and expose provider behavior through traits. Svelte/TypeScript uses Prettier with `prettier-plugin-svelte`; components use PascalCase filenames, helpers use kebab-case filenames, and Tailwind v4 theme tokens should be preferred over repeated arbitrary colors.

## Testing Guidelines

Add Rust tests alongside the relevant crate or under `tests/` and run `cargo test --workspace` (or a focused package command). Frontend changes must pass `svelte-check`; production-facing changes should also pass the web build. Name tests after observable behavior, for example `rejects_cross_organization_access`.

## Commit & Pull Request Guidelines

Use short, imperative commit subjects matching the existing history, such as `Extract provider and operation app actions`. Keep commits focused and explain behavior changes in the body when needed. Pull requests should describe the affected module, database/configuration changes, verification commands, and rollout notes. Include screenshots for visible UI changes, and call out credential or authorization changes.

## Security & Configuration Tips

Do not commit secrets, local `.env` files, provider credentials, or generated build artifacts. Use the documented admin CLI for first-time initialization and recovery. Treat provider credentials as sensitive, preserve audit events, and verify organization/RBAC boundaries when adding endpoints or operations.
