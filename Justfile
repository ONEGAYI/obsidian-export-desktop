towncrier_cmd := "uvx towncrier==24.8.0"

# --- Desktop app (Tauri) -----------------------------------------------------

# Git Bash on Windows: plain lines run via `bash -c` (shebang recipes hit a
# temp-file path bug in just 1.53 on Windows, so keep recipes single-line).
set windows-shell := ["bash.exe", "-c"]

exe := if os() == "windows" { ".exe" } else { "" }
triple := `rustc -vV | sed -n 's/^host: //p'`

# Build the CLI and copy it into the desktop app's sidecar location
desktop-sync-sidecar:
    cargo build --release --bin obsidian-export && mkdir -p desktop/src-tauri/binaries && cp "target/release/obsidian-export{{exe}}" "desktop/src-tauri/binaries/obsidian-export-{{triple}}{{exe}}" && echo "Sidecar synced."

# Run the desktop app in dev mode (syncs the sidecar first)
desktop-dev: desktop-sync-sidecar
    cd desktop && pnpm tauri dev

# Build a distributable desktop bundle (syncs the sidecar first)
desktop-build: desktop-sync-sidecar
    cd desktop && pnpm tauri build

# Run the desktop crate's Rust tests
desktop-test:
    cargo test --manifest-path desktop/src-tauri/Cargo.toml

# -----------------------------------------------------------------------------

_default:
    @{{just_executable()}} --choose

# Add a new changelog entry using towncrier
add-changelog:
    {{towncrier_cmd}} create --edit
    git add changelog.d

# Display the changelog that would be generated on the next release
preview-changelog:
    {{towncrier_cmd}} build --draft --version $(just _get-next-version-number)

# Create a new release
make-new-release:
    #!/usr/bin/env bash
    set -euo pipefail

    git add .
    if ! git diff-index --quiet HEAD; then
        printf "Working directory is not clean. Please commit or stash your changes.\n"
        exit 1
    fi

    VERSION=$(just _get-next-version-number)
    COMMITMSG=$(mktemp --tmpdir commitmsg.XXXXXXXXXX)
    trap 'rm "$COMMITMSG"' EXIT
    set -x

    cargo set-version "${VERSION}"

    # Construct a git commit message.
    # This must be done before the next step so we can leverage the --draft
    # flag here to get a list of changes being introduced by this release.
    printf "Release v${VERSION}\n\n" > "$COMMITMSG"
    {{towncrier_cmd}} build --draft --version "${VERSION}" >> "$COMMITMSG"

    # Generate changelog and docs
    {{towncrier_cmd}} build --version "${VERSION}"
    docs/generate.sh

    # Stage all the changes we've prepared
    git add .
    # There are likely trailing whitespace changes in the changelog, but a single
    # run of pre-commit will fix these automatically.
    pre-commit run || git add .

    git commit --file "$COMMITMSG"
    git tag "v${VERSION}"

    set +x
    printf "\n\nSuccessfully created release %s\n" "v${VERSION}"
    printf "\nYou'll probably want to continue with:\n"
    printf "\tgit push origin main %s\n" "v${VERSION}"

_get-next-version-number:
    #!/usr/bin/env bash
    set -euo pipefail

    DATEPART=$(date +%y.%-m)
    ITERATION=0

    while true; do
        VERSION_STRING="${DATEPART}.${ITERATION}"
        if git rev-list "v$VERSION_STRING" > /dev/null 2>&1; then
            ((ITERATION++))
        else
            printf "$VERSION_STRING"
            exit
        fi
    done
