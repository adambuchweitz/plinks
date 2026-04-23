# plinks

`plinks` is a project-local link manager. When used in a repository, check in `project-links.toml` so everyone on the team can open the same docs, dashboards, and tickets from either a CLI or an interactive `ratatui` TUI.

## Why you might want it

- Keep useful or common links in the repo instead of in browser bookmarks.
- Make onboarding easier (the same links for everyone, in the same place).
- Avoid hunting through wikis, chat logs, and stale docs when you need the right dashboard or ticket board.

## Quick start

1. Add a link from anywhere inside your repo:

```bash
plinks add git https://github.com/my-username/my-project
plinks a slack https://my-org.slack.com --alias chat --tag comms # shorthand for "add"
plinks a linear https://linear.app/my-org --alias pm --tag comms
plinks a docs https://docs.rs --alias api --tag rust --tag reference --note "Rust API docs"
```

If `project-links.toml` doesn't exist yet, `plinks` creates it at your Git repository root (or in the current directory if you're not in a Git repo). Commit the file to share it with your team.

2. Open links by name, alias, or tag:

```bash
plinks open docs
plinks o api # shorthand for "open"
plinks o --tag rust
```

3. List links:

```bash
plinks list
plinks ls --tag rust # shorthand for "list"
```

4. Launch the interactive TUI:

```bash
plinks
plinks manage # or explicitly
```

In the TUI, press `y` to copy the highlighted link to your system clipboard.
On Linux, `plinks` uses `wl-copy`, `xclip`, or `xsel` so the copied URL persists after `plinks` exits. At least one of those utilities must be installed for `y` to work; otherwise the yank action fails with an error.

Run `plinks --help` to see all commands.

## How `plinks` finds `project-links.toml`

`plinks` looks for `project-links.toml` in the current directory first.

If it does not find one, it checks ancestor directories up to the Git repository root:

- If an ancestor already contains `project-links.toml`, that file is used.
- If no file exists yet, `plinks` uses `<git-root>/project-links.toml`.
- Outside a Git repository, it falls back to `<cwd>/project-links.toml`.

This makes it practical to run `plinks` anywhere inside a repository while still keeping one shared config file at the project level.

## Config format

```toml
[links]

[links.docs]
url = "https://docs.rs"
aliases = ["api"]
tags = ["reference", "rust"]
note = "Rust API docs"

[links.jira]
url = "https://jira.example.com/browse/PROJ"
tags = ["ops"]
```

Primary names, aliases, and tags are normalized when saved to lowercase and may contain letters, numbers, `_`, and `-`.

## Install

### Prebuilt binaries (GitHub Releases)

Prebuilt binaries are published on GitHub Releases for these targets:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`

Windows releases are unsigned, so depending on local policy, Windows may show SmartScreen or other trust warnings before first launch.

After downloading:

- Unpack the archive and put `plinks`/`plinks.exe` somewhere on your `PATH`.
- On macOS/Linux you may need `chmod +x plinks` after extracting.

### From source (Cargo)

Install the latest from this repo:

```bash
cargo install --git https://github.com/adambuchweitz/plinks
```

Or install from a local checkout:

```bash
cargo install --path .
```

## Development

Build a development binary from the checkout:

```bash
cargo build
```

Build an optimized release binary:

```bash
cargo build --release
```

The compiled binary is written to:

- `target/debug/plinks` for development builds
- `target/release/plinks` for release builds

Run the binary directly from the checkout:

```bash
cargo run -- <command>
```

Test:

```bash
cargo test
```

Install the repository's Git hooks:

```bash
./scripts/install-git-hooks.sh
```

The pre-commit hook runs the same lint commands as CI:

```bash
./scripts/run-linters.sh
```

## Releases

GitHub Releases publish prebuilt binaries for Windows, Linux, and macOS. Release assets are named using stable target-specific archives:

- `plinks-v<version>-x86_64-pc-windows-msvc.zip`
- `plinks-v<version>-x86_64-unknown-linux-gnu.tar.gz`
- `plinks-v<version>-x86_64-apple-darwin.tar.gz`

Every release also includes a `SHA256SUMS` file covering all published archives.

## Maintainer Release Process

1. Bump the crate version in `Cargo.toml` and refresh `Cargo.lock` so locked CI builds stay in sync.
2. Merge the release commit to `main`.
3. Create and push a matching Git tag in the form `vX.Y.Z`.
4. GitHub Actions handles the rest:
    - Validates that the tag matches `Cargo.toml`
    - Builds the release binaries
    - Runs `--help` smoke tests for each release target
    - Packages the binary together with `LICENSE` and `README.md`
    - Generates `SHA256SUMS`
    - Publishes the release assets automatically

## Arch Linux Packaging for AUR

Build the Arch distribution artifacts:

```bash
./scripts/build-arch-package.sh
```

This writes the source tarball and `PKGBUILD` to `dist/arch/`.

Build the package locally with `makepkg`:

```bash
cd dist/arch
makepkg -Cf
```

Build and install the package with `makepkg`:

```bash
cd dist/arch
makepkg -Csi
```

`./scripts/build-arch-package.sh` removes previously built `pkg.tar.*` artifacts in `dist/arch/`, so rerunning this sequence rebuilds the package instead of reusing an older archive.

## License

MIT. See [LICENSE](LICENSE).
