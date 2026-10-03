// scripts/sign-release.js
//
// Signs the release exe and msi for the in-app updater, writing a .sig next to
// each. Upload both .sig files to the GitHub release alongside the binaries.
//
// Usage (after `npm run tauri build`):
//   npm run sign-release
//
// Key path defaults to %USERPROFILE%\.mhm\update.key; override with
// TAURI_SIGNING_PRIVATE_KEY_PATH. The key password is prompted for unless
// TAURI_SIGNING_PRIVATE_KEY_PASSWORD is set.

import { readFileSync, existsSync } from "node:fs";
import { execSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { version } = JSON.parse(readFileSync(path.join(rootDir, "src-tauri", "tauri.conf.json"), "utf-8"));
const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH ?? path.join(os.homedir(), ".mhm", "update.key");
const release = path.join(rootDir, "src-tauri", "target", "release");
const files = [
  path.join(release, "mh-multiverse.exe"),
  path.join(release, "bundle", "msi", `MH Multiverse_${version}_x64_en-US.msi`),
];

if (!existsSync(keyPath)) {
  console.error(`Signing key not found: ${keyPath}`);
  process.exit(1);
}
for (const file of files) {
  if (!existsSync(file)) {
    console.error(`Missing build output: ${file}\nRun \`npm run tauri build\` first.`);
    process.exit(1);
  }
}
for (const file of files) {
  console.log(`Signing ${path.basename(file)}`);
  // Paths are quoted by hand: the msi name has a space.
  execSync(`npx tauri signer sign -f "${keyPath}" "${file}"`, { cwd: rootDir, stdio: "inherit" });
}
console.log(`\nUpload with the release:\n${files.map(f => `  ${f}\n  ${f}.sig`).join("\n")}`);
