import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const bump = process.argv[2];
if (!["major", "minor", "patch"].includes(bump)) {
  console.error(`Usage: node scripts/bump-version.mjs <major|minor|patch>`);
  process.exit(1);
}

const pkgPath = join(root, "package.json");
const confPath = join(root, "src-tauri", "tauri.conf.json");
const cargoPath = join(root, "src-tauri", "Cargo.toml");

const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const current = pkg.version;

const parts = current.split(".").map(Number);
if (parts.length !== 3 || parts.some(Number.isNaN)) {
  console.error(`Cannot parse version "${current}" in package.json`);
  process.exit(1);
}

let [major, minor, patch] = parts;
if (bump === "major") {
  major += 1;
  minor = 0;
  patch = 0;
} else if (bump === "minor") {
  minor += 1;
  patch = 0;
} else {
  patch += 1;
}

const next = `${major}.${minor}.${patch}`;

pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.version = next;
writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

// Only the first `version = ` line belongs to [package]; dependency versions must stay untouched.
const cargo = readFileSync(cargoPath, "utf8");
if (!/^version = .*$/m.test(cargo)) {
  console.error("No version field found in src-tauri/Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoPath, cargo.replace(/^version = .*$/m, `version = "${next}"`));

console.log(`Bumped ${current} -> ${next}`);
