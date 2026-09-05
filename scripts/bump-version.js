// scripts/bump-version.mjs
//
// Bumps the version across package.json, src-tauri/Cargo.toml, and
// src-tauri/tauri.conf.json, then syncs package-lock.json and Cargo.lock.
//
// Usage:
//   npm run bump-version -- patch
//   npm run bump-version -- minor
//   npm run bump-version -- major
//   npm run bump-version -- 1.4.0

import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, "..");

const packageJsonPath = path.join(rootDir, "package.json");
const cargoTomlPath = path.join(rootDir, "src-tauri", "Cargo.toml");
const tauriConfPath = path.join(rootDir, "src-tauri", "tauri.conf.json");

const VERSION_REGEX = /^\d+\.\d+\.\d+$/;
const BUMP_TYPES = ["patch", "minor", "major"];

function getPackageSection(cargoTomlContent) {
  const match = cargoTomlContent.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  if (!match) {
    throw new Error("Could not find [package] section in Cargo.toml");
  }
  return match[0];
}

function extractCargoTomlVersion(cargoTomlContent) {
  const section = getPackageSection(cargoTomlContent);
  const match = section.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error('Could not find "version" field in [package] section of Cargo.toml');
  }
  return match[1];
}

function bumpCargoToml(cargoTomlContent, newVersion) {
  const section = getPackageSection(cargoTomlContent);
  const updatedSection = section.replace(
    /^version\s*=\s*"[^"]+"/m,
    `version = "${newVersion}"`
  );
  return cargoTomlContent.replace(section, updatedSection);
}

function replaceFirstJsonVersion(jsonContent, newVersion, label) {
  const regex = /"version":\s*"[^"]+"/;
  if (!regex.test(jsonContent)) {
    throw new Error(`Could not find "version" field in ${label}`);
  }
  return jsonContent.replace(regex, `"version": "${newVersion}"`);
}

function computeBumpedVersion(current, bumpType) {
  const parts = current.split(".").map(Number);
  if (parts.length !== 3 || parts.some((n) => Number.isNaN(n))) {
    throw new Error(`Current version "${current}" is not in major.minor.patch format`);
  }
  let [major, minor, patch] = parts;
  if (bumpType === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (bumpType === "minor") {
    minor += 1;
    patch = 0;
  } else {
    patch += 1;
  }
  return `${major}.${minor}.${patch}`;
}

function runSyncStep(label, fn) {
  console.log(`\nSyncing: ${label}`);
  try {
    fn();
    console.log(`  done: ${label}`);
  } catch (err) {
    console.error(`  failed: ${label}`);
    console.error(`  ${err.message}`);
    console.error(`  Run it manually once the underlying issue is fixed.`);
  }
}

function main() {
  const arg = process.argv[2];
  if (!arg) {
    throw new Error("Usage: npm run bump-version -- <patch|minor|major|x.y.z>");
  }

  const isExplicit = VERSION_REGEX.test(arg);
  const isBumpType = BUMP_TYPES.includes(arg);
  if (!isExplicit && !isBumpType) {
    throw new Error(
      `Argument "${arg}" is not a valid bump type (${BUMP_TYPES.join("|")}) or version (x.y.z)`
    );
  }

  const packageJsonRaw = readFileSync(packageJsonPath, "utf-8");
  const cargoTomlRaw = readFileSync(cargoTomlPath, "utf-8");
  const tauriConfRaw = readFileSync(tauriConfPath, "utf-8");

  const packageJsonVersion = JSON.parse(packageJsonRaw).version;
  const tauriConfVersion = JSON.parse(tauriConfRaw).version;
  const cargoTomlVersion = extractCargoTomlVersion(cargoTomlRaw);

  if (packageJsonVersion !== tauriConfVersion || packageJsonVersion !== cargoTomlVersion) {
    throw new Error(
      "Version fields are already out of sync, refusing to bump automatically:\n" +
        `  package.json:     ${packageJsonVersion}\n` +
        `  Cargo.toml:        ${cargoTomlVersion}\n` +
        `  tauri.conf.json:   ${tauriConfVersion}\n` +
        "Reconcile these manually before running this script."
    );
  }

  const currentVersion = packageJsonVersion;
  const newVersion = isExplicit ? arg : computeBumpedVersion(currentVersion, arg);

  if (newVersion === currentVersion) {
    console.warn(`Warning: new version (${newVersion}) is the same as the current version.`);
  }

  writeFileSync(packageJsonPath, replaceFirstJsonVersion(packageJsonRaw, newVersion, "package.json"));
  writeFileSync(tauriConfPath, replaceFirstJsonVersion(tauriConfRaw, newVersion, "tauri.conf.json"));
  writeFileSync(cargoTomlPath, bumpCargoToml(cargoTomlRaw, newVersion));

  console.log(`Version bumped: ${currentVersion} -> ${newVersion}`);
  console.log("  package.json");
  console.log("  src-tauri/Cargo.toml");
  console.log("  src-tauri/tauri.conf.json");

  runSyncStep("npm install --package-lock-only", () => {
    execSync("npm install --package-lock-only", { cwd: rootDir, stdio: "inherit" });
  });

  runSyncStep("cargo check", () => {
    execSync("cargo check", { cwd: path.join(rootDir, "src-tauri"), stdio: "inherit" });
  });
}

try {
  main();
} catch (err) {
  console.error(`Error: ${err.message}`);
  process.exit(1);
}
