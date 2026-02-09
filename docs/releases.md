# Release Automation

This project includes optional release automation using [git-cliff](https://git-cliff.org/) for changelog generation and semantic versioning.

## Overview

The release system has two modes:

1. **Manual releases** — Use `just` recipes to control when releases happen
2. **Automatic releases** — Merge to main triggers version detection and tagging

Both modes use conventional commits to determine version bumps:

| Commit type | Version bump |
|-------------|--------------|
| `fix:` | Patch (0.0.X) |
| `feat:` | Minor (0.X.0) |
| `feat!:` or `BREAKING CHANGE:` | Major (X.0.0) |

## Quick Start

### Manual Release

```bash
# Preview what would be released
just changelog-preview

# Bump version automatically based on commits
just bump

# Or specify an exact version
just release v1.2.3
```

### Automatic Release

Enable automatic releases by setting a repository variable:

```bash
gh variable set AUTO_RELEASE_ENABLED --body "true"
```

Now when you merge PRs to main, the release workflow will:

1. Analyze commits since the last tag
2. Calculate the next semantic version
3. Create and push a tag
4. Trigger the CD workflow to build and publish

## Justfile Recipes

| Recipe | Description |
|--------|-------------|
| `just changelog` | Generate CHANGELOG.md from all commits |
| `just changelog-preview` | Preview unreleased changes (dry run) |
| `just bump` | Auto-calculate next version, update Cargo.toml and CHANGELOG |
| `just release v1.2.3` | Full release with pre-checks, changelog, and tagging |
| `just release-check` | Run pre-release validation (tests, clippy, deny) |

### Example: Manual Release Flow

```bash
# 1. Preview the release
just changelog-preview

# 2. Run pre-release checks
just release-check

# 3. Create the release
just release v1.0.0

# 4. Push to trigger CD
git push && git push --tags
```

## GitHub Workflows

### release.yml

Runs on every push to `main`. Checks if commits warrant a release.

**Controlled by repository variables:**

| Variable | Purpose |
|----------|---------|
| `AUTO_RELEASE_ENABLED` | Set to `true` to enable automatic releases |
| `AUTO_RELEASE_DRY_RUN` | Set to `true` to log without creating tags |

### cd.yml

Triggered by version tags (`v*.*.*`). Builds binaries for multiple platforms and optionally publishes to package registries.

**Build matrix:**

- Linux: x64/arm64, glibc/musl
- macOS: x64/arm64
- Windows: x64/arm64 MSVC

**Optional publishing (controlled by repository variables):**

| Variable | Purpose | Required Secrets |
|----------|---------|------------------|
| `CRATES_IO_ENABLED` | Publish to crates.io | `CARGO_TOKEN` |
| `HOMEBREW_ENABLED` | Update Homebrew formula | `HOMEBREW_COMMITTER_TOKEN` |
| `DEB_ENABLED` | Build Debian packages | — |
| `RPM_ENABLED` | Build RPM packages | — |
| `NPM_ENABLED` | Publish to npm (OIDC) | — (uses trusted publishing) |
| `SBOM_ENABLED` | Generate CycloneDX SBOM | — |
| `GPG_SIGNING_ENABLED` | Sign artifacts | `GPG_RELEASE_KEY`, `GPG_PASSPHRASE` |

## Configuration

### cliff.toml

The git-cliff configuration file controls changelog generation:

- Parses conventional commits
- Groups by type (Features, Bug Fixes, etc.)
- Links to GitHub PRs and contributors
- Supports GitHub usernames via `--github-repo`

Customize sections by editing the `commit_parsers` array.

### Setting Up Secrets

For crates.io publishing:

```bash
gh secret set CARGO_TOKEN
```

For GPG signing — the armored private key must be base64-encoded before storing as a secret (GitHub secrets strip newlines, which breaks PEM formatting):

```bash
gpg --export-secret-keys --armor YOUR_KEY_ID | base64 | gh secret set GPG_RELEASE_KEY --app actions
gh secret set GPG_PASSPHRASE
```

For Homebrew (requires a PAT with `repo` and `workflow` scopes on your tap repo):

```bash
gh secret set HOMEBREW_COMMITTER_TOKEN
```

### Setting Up Homebrew Distribution

The CD workflow generates a multi-platform Homebrew formula from release artifacts and pushes it to your tap via the GitHub API. Users get pre-built binaries — no compilation required.

1. Create a tap repository (e.g., `yourname/homebrew-brew`)

2. Create an initial placeholder formula. The CD workflow will overwrite it on first release, but the file must exist so `brew tap` can discover it. A minimal placeholder:

```ruby
# typed: true
# frozen_string_literal: true

class YourTool < Formula
  desc "Your tool description"
  homepage "https://github.com/yourname/your-tool"
  version "0.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yourname/your-tool/releases/download/v#{version}/your-tool-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "placeholder"
    else
      url "https://github.com/yourname/your-tool/releases/download/v#{version}/your-tool-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "placeholder"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yourname/your-tool/releases/download/v#{version}/your-tool-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "placeholder"
    else
      url "https://github.com/yourname/your-tool/releases/download/v#{version}/your-tool-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "placeholder"
    end
  end

  def install
    bin.install "bin/your-tool"
    man1.install Dir["share/man/man1/*.1"]
    bash_completion.install "share/completions/your-tool.bash" => "your-tool"
    zsh_completion.install "share/completions/_your-tool"
    fish_completion.install "share/completions/your-tool.fish"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/your-tool --version")
  end
end
```

3. On each release, the CD workflow downloads the `.sha256` files from the release, generates the full formula with correct checksums, and pushes it to the tap via the GitHub API.

4. Enable the workflow:

```bash
gh variable set HOMEBREW_ENABLED --body "true"
```

### Setting Up Debian Package Distribution

[cargo-deb](https://github.com/kornelski/cargo-deb) builds `.deb` packages from Cargo metadata.

1. Add metadata to your `Cargo.toml`:

```toml
[package.metadata.deb]
maintainer = "Your Name <your-email@example.com>"
copyright = "Your Name"
license-file = ["LICENSE-MIT", "4"]
extended-description = """\
Your tool description"""
section = "utility"
priority = "optional"
assets = [
    # Binary
    ["target/release/your-tool", "usr/bin/", "755"],
    # Man pages (if using xtask)
    ["target/dist/share/man/man1/*", "usr/share/man/man1/", "644"],
    # Shell completions
    ["target/dist/share/completions/your-tool.bash", "usr/share/bash-completion/completions/your-tool", "644"],
    ["target/dist/share/completions/_your-tool", "usr/share/zsh/vendor-completions/", "644"],
    ["target/dist/share/completions/your-tool.fish", "usr/share/fish/vendor_completions.d/", "644"],
]
```

2. Enable the workflow:

```bash
gh variable set DEB_ENABLED --body "true"
```

### Setting Up RPM Package Distribution

[cargo-generate-rpm](https://github.com/cat-in-136/cargo-generate-rpm) builds `.rpm` packages from Cargo metadata.

1. Add metadata to your `Cargo.toml`:

```toml
[package.metadata.generate-rpm]
assets = [
    # Binary
    { source = "target/release/your-tool", dest = "/usr/bin/your-tool", mode = "755" },
    # Man pages (if using xtask)
    { source = "target/dist/share/man/man1/*", dest = "/usr/share/man/man1/", mode = "644", doc = true },
    # Shell completions
    { source = "target/dist/share/completions/your-tool.bash", dest = "/usr/share/bash-completion/completions/your-tool", mode = "644" },
    { source = "target/dist/share/completions/_your-tool", dest = "/usr/share/zsh/site-functions/_your-tool", mode = "644" },
    { source = "target/dist/share/completions/your-tool.fish", dest = "/usr/share/fish/vendor_completions.d/your-tool.fish", mode = "644" },
]

[package.metadata.generate-rpm.requires]
# Add runtime dependencies here if needed
# glibc = ">= 2.17"
```

2. Enable the workflow:

```bash
gh variable set RPM_ENABLED --body "true"
```

### Setting Up npm Distribution

npm distribution uses platform-specific packages with a wrapper package that handles binary resolution. This approach (inspired by [Sentry's strategy](https://sentry.engineering/blog/publishing-binaries-on-npm)) ensures compatibility across all npm environments.

#### Authentication: Trusted Publishing (OIDC)

The CD workflow uses [npm trusted publishing](https://docs.npmjs.com/trusted-publishers) instead of long-lived tokens. This uses GitHub Actions' OIDC identity to authenticate with npm — no secrets to store or rotate.

**First-time bootstrap** (one-time manual step, because npm requires packages to exist before you can configure trusted publishing):

1. Create a [granular access token](https://docs.npmjs.com/about-access-tokens) on npmjs.com scoped to your packages with Publish permission. This bypasses 2FA for the initial publish.

2. Publish all packages manually:

```bash
# Set the token
export NODE_AUTH_TOKEN="your-granular-token"

# Publish platform packages first
for dir in npm/platforms/*/; do
    cd "$dir" && npm publish --access public && cd -
done

# Then the main wrapper
cd npm/your-tool && npm publish --access public
```

3. Configure trusted publishing on each package at npmjs.com:
   - Go to package settings → Trusted Publishers → Add
   - Repository owner: `yourname`
   - Repository name: `your-tool`
   - Workflow filename: `cd.yml`
   - Environment: (leave blank)

4. Delete the granular access token — you won't need it again.

5. Enable the workflow:

```bash
gh variable set NPM_ENABLED --body "true"
```

From this point on, `cd.yml` authenticates via OIDC and publishes with `--provenance`, which generates signed provenance attestations automatically.

**Requirements:** npm >= 11.5.1 (the workflow installs `npm@latest` since Node 20 ships with npm 10.x). Only cloud-hosted GitHub Actions runners are supported — self-hosted runners cannot use OIDC trusted publishing.

#### Package Structure

```
npm/
├── your-tool/                    # Main wrapper package
│   ├── package.json
│   ├── index.js                  # Binary resolution
│   ├── cli.js                    # CLI entry point
│   └── install.js                # Postinstall fallback
└── platforms/
    ├── your-tool-darwin-arm64/
    │   ├── package.json
    │   └── bin/.gitkeep
    ├── your-tool-darwin-x64/
    ├── your-tool-linux-arm64/
    ├── your-tool-linux-x64/
    ├── your-tool-win32-arm64/
    └── your-tool-win32-x64/
```

Platform `bin/` directories contain `.gitkeep` placeholders. The CD workflow extracts actual binaries from release tarballs at publish time.

#### Platform Package (e.g., `npm/platforms/your-tool-linux-x64/package.json`)

```json
{
  "name": "@yourscope/your-tool-linux-x64",
  "version": "0.1.0",
  "description": "Your tool description (linux-x64 binary)",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/yourname/your-tool"
  },
  "os": ["linux"],
  "cpu": ["x64"],
  "files": ["bin/"]
}
```

Valid `os` values: `"linux"`, `"darwin"`, `"win32"`
Valid `cpu` values: `"x64"`, `"arm64"`

#### Main Wrapper Package (`npm/your-tool/package.json`)

```json
{
  "name": "@yourscope/your-tool",
  "version": "0.1.0",
  "description": "Your tool description",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/yourname/your-tool"
  },
  "bin": {
    "your-tool": "cli.js"
  },
  "scripts": {
    "postinstall": "node install.js"
  },
  "files": ["index.js", "cli.js", "install.js"],
  "optionalDependencies": {
    "@yourscope/your-tool-darwin-arm64": "0.1.0",
    "@yourscope/your-tool-darwin-x64": "0.1.0",
    "@yourscope/your-tool-linux-arm64": "0.1.0",
    "@yourscope/your-tool-linux-x64": "0.1.0",
    "@yourscope/your-tool-win32-arm64": "0.1.0",
    "@yourscope/your-tool-win32-x64": "0.1.0"
  }
}
```

#### Binary Resolution (`npm/your-tool/index.js`)

```javascript
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "@yourscope/your-tool-darwin-arm64",
  "darwin-x64": "@yourscope/your-tool-darwin-x64",
  "linux-arm64": "@yourscope/your-tool-linux-arm64",
  "linux-x64": "@yourscope/your-tool-linux-x64",
  "win32-arm64": "@yourscope/your-tool-win32-arm64",
  "win32-x64": "@yourscope/your-tool-win32-x64",
};

function getBinaryPath() {
  const platformKey = `${process.platform}-${process.arch}`;
  const packageName = PLATFORMS[platformKey];

  if (!packageName) {
    throw new Error(`Unsupported platform: ${platformKey}`);
  }

  const binaryName = process.platform === "win32" ? "your-tool.exe" : "your-tool";

  // Try optionalDependency first
  try {
    const packagePath = require.resolve(`${packageName}/package.json`);
    const binaryPath = path.join(path.dirname(packagePath), "bin", binaryName);
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch {}

  // Fall back to postinstall location
  const fallbackPath = path.join(__dirname, "bin", binaryName);
  if (fs.existsSync(fallbackPath)) {
    return fallbackPath;
  }

  throw new Error(
    `Could not find your-tool binary. ` +
    `Try reinstalling @yourscope/your-tool`
  );
}

module.exports = { getBinaryPath };
```

#### CLI Entry Point (`npm/your-tool/cli.js`)

```javascript
#!/usr/bin/env node
const { spawn } = require("child_process");
const { getBinaryPath } = require("./index.js");

const child = spawn(getBinaryPath(), process.argv.slice(2), {
  stdio: "inherit",
});

child.on("close", (code) => process.exit(code ?? 0));
```

#### Postinstall Fallback (`npm/your-tool/install.js`)

```javascript
const https = require("https");
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const PLATFORMS = {
  "darwin-arm64": "@yourscope/your-tool-darwin-arm64",
  "darwin-x64": "@yourscope/your-tool-darwin-x64",
  "linux-arm64": "@yourscope/your-tool-linux-arm64",
  "linux-x64": "@yourscope/your-tool-linux-x64",
  "win32-arm64": "@yourscope/your-tool-win32-arm64",
  "win32-x64": "@yourscope/your-tool-win32-x64",
};

async function install() {
  const platformKey = `${process.platform}-${process.arch}`;
  const packageName = PLATFORMS[platformKey];

  if (!packageName) {
    console.warn(`Unsupported platform: ${platformKey}`);
    return;
  }

  // Check if optionalDependency already installed
  try {
    require.resolve(`${packageName}/package.json`);
    return; // Already installed
  } catch {}

  console.log(`Downloading ${packageName}...`);

  const version = require("./package.json").version;
  const tarballUrl = `https://registry.npmjs.org/${packageName}/-/${packageName.split("/")[1]}-${version}.tgz`;

  const tarball = await download(tarballUrl);
  const files = extractTar(zlib.gunzipSync(tarball));

  const binaryName = process.platform === "win32" ? "your-tool.exe" : "your-tool";
  const binaryEntry = files.find((f) => f.name.endsWith(`/bin/${binaryName}`));

  if (!binaryEntry) {
    throw new Error("Binary not found in package");
  }

  const binDir = path.join(__dirname, "bin");
  fs.mkdirSync(binDir, { recursive: true });
  fs.writeFileSync(path.join(binDir, binaryName), binaryEntry.data, { mode: 0o755 });
}

function download(url) {
  return new Promise((resolve, reject) => {
    https.get(url, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        return download(res.headers.location).then(resolve, reject);
      }
      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    });
  });
}

function extractTar(buffer) {
  const files = [];
  let offset = 0;

  while (offset < buffer.length - 512) {
    const header = buffer.slice(offset, offset + 512);
    if (header[0] === 0) break;

    const name = header.slice(0, 100).toString().replace(/\0/g, "");
    const size = parseInt(header.slice(124, 136).toString(), 8);

    offset += 512;
    if (size > 0) {
      files.push({ name, data: buffer.slice(offset, offset + size) });
      offset += Math.ceil(size / 512) * 512;
    }
  }

  return files;
}

install().catch((err) => {
  console.error("Failed to install binary:", err.message);
  process.exit(1);
});
```

## Build Features

### Release Tarball Structure

Each release tarball contains (paths are relative to the archive root):

```
bin/
└── your-tool               # The compiled binary
share/
│   ├── man/
│   │   └── man1/
│   │       ├── your-tool.1          # Main command man page
│   │       └── your-tool-*.1        # Subcommand man pages
│   └── completions/
│       ├── your-tool.bash           # Bash completions
│       ├── your-tool.fish           # Fish completions
│       ├── your-tool.ps1            # PowerShell completions
│       └── _your-tool               # Zsh completions
LICENSE-*
README.md
CHANGELOG.md
```

This structure follows [XDG conventions](https://specifications.freedesktop.org/basedir-spec/latest/) and is compatible with Homebrew's standard installation methods. The tarball has no top-level directory — files are at the root.

### cargo-auditable

All release builds use [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable) to embed dependency information in the binary. This enables vulnerability scanning of deployed binaries:

```bash
cargo audit bin ./target/release/your-tool
```

### CycloneDX SBOM

When `SBOM_ENABLED=true`, builds generate a [CycloneDX](https://cyclonedx.org/) Software Bill of Materials in JSON format. This is useful for supply chain security and compliance.

## Pre-release Versions

Tags containing a hyphen are treated as pre-releases:

- `v1.0.0` → Full release
- `v1.0.0-beta.1` → Pre-release (marked as such on GitHub)

Pre-releases skip crates.io, Homebrew, and npm publishing but still build and upload binaries.

## Troubleshooting

### "No version bump detected"

git-cliff found no conventional commits since the last tag. Ensure commits follow the format:

```
feat: add new feature
fix: resolve bug
docs: update readme
```

### Release workflow not triggering

Check that:

1. `AUTO_RELEASE_ENABLED` is set to exactly `true` (case-sensitive)
2. You're pushing to the `main` branch
3. The workflow has `contents: write` permission

### CD workflow skipping jobs

Most publishing jobs require their respective `*_ENABLED` variable to be set. Check:

```bash
gh variable list
```

### GPG signing fails with "base64: invalid input"

The `GPG_RELEASE_KEY` secret must contain the base64-encoded armored key, not the raw armored key. GitHub secrets strip newlines, which breaks PEM formatting. Encode before storing:

```bash
gpg --export-secret-keys --armor KEY_ID | base64 | gh secret set GPG_RELEASE_KEY --app actions
```

### Homebrew formula not updating

The CD workflow pushes to the tap via the GitHub API using `HOMEBREW_COMMITTER_TOKEN`. Verify that:

1. The token has `repo` and `workflow` scopes
2. The formula file exists in `Formula/your-tool.rb` in the tap repo
3. The release includes `.sha256` files for all platform tarballs

### npm publish fails with 401/403

If using trusted publishing (OIDC), verify that:

1. Each package has a trusted publisher configured on npmjs.com pointing to your repo and workflow
2. The workflow has `id-token: write` permission
3. You're using npm >= 11.5.1 (the workflow installs `npm@latest`)
4. You're running on a cloud-hosted GitHub Actions runner (self-hosted runners don't support OIDC)
