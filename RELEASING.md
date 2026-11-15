# Releasing

Published to crates.io as `vaartix` by GitHub Actions with crates.io trusted publishing: no API token is stored.

## One-time setup

1. Sign in to crates.io with GitHub (the vaartix-mail account) and verify an email address.
2. The first release must be published once by hand (trusted publishing needs the crate to exist):
   `cargo login` (a short-lived token, deleted afterwards) then `cargo publish`.
3. crates.io → the `vaartix` crate → **Settings → Trusted Publishing → Add**: repository
   `vaartix-mail/vaartix-rust`, workflow `publish.yml`, environment `crates-io`. Then delete the token.
4. GitHub → Settings → Environments → `crates-io`: required reviewer, deployment tags `v*` only.

## Each release

1. Update `version` in `Cargo.toml` and move the "Unreleased" notes in `CHANGELOG.md`. Merge to `main`.
2. Create a GitHub release with the tag `v<version>` on `main`, then approve the publish run.
