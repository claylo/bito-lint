const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "@claylo/bito-lint-darwin-arm64",
  "darwin-x64": "@claylo/bito-lint-darwin-x64",
  "linux-arm64": "@claylo/bito-lint-linux-arm64",
  "linux-x64": "@claylo/bito-lint-linux-x64",
  "win32-arm64": "@claylo/bito-lint-win32-arm64",
  "win32-x64": "@claylo/bito-lint-win32-x64",
};

function getBinaryPath() {
  const platformKey = `${process.platform}-${process.arch}`;
  const packageName = PLATFORMS[platformKey];

  if (!packageName) {
    throw new Error(`Unsupported platform: ${platformKey}`);
  }

  const binaryName =
    process.platform === "win32" ? "bito-lint.exe" : "bito-lint";

  // Try optionalDependency first
  try {
    const packagePath = require.resolve(`${packageName}/package.json`);
    const binaryPath = path.join(path.dirname(packagePath), "bin", binaryName);
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch {
    // optionalDependency not installed, fall through to fallback
  }

  // Fall back to postinstall location
  const fallbackPath = path.join(__dirname, "bin", binaryName);
  if (fs.existsSync(fallbackPath)) {
    return fallbackPath;
  }

  throw new Error(
    `Could not find bito-lint binary. ` +
      `Try reinstalling @claylo/bito-lint`
  );
}

module.exports = { getBinaryPath };
