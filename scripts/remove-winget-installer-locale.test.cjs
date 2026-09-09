const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, readFileSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { spawnSync } = require('node:child_process');

test('removes upgrade language restrictions without changing installer identity or metadata', () => {
  const directory = mkdtempSync(join(tmpdir(), 'sqlplan-winget-locale-'));
  const path = join(directory, 'PsyChonek.SqlPlanForDummies.installer.yaml');
  const base = readFileSync(join(__dirname, '../winget/PsyChonek.SqlPlanForDummies.installer.yaml'), 'utf8');
  const run = () => spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
    join(__dirname, 'remove-winget-installer-locale.ps1'), '-Path', path], { encoding: 'utf8' });
  try {
    // Komac can hoist the shared locale or write it on individual installers.
    for (const input of [
      base.replace('Platform:', 'InstallerLocale: en-US\nPlatform:'),
      base.replace(/(- Architecture: [^\r\n]+)(\r?\n)/g, '$1$2  InstallerLocale: en-US$2'),
    ]) {
      writeFileSync(path, input);
      const result = run();
      assert.equal(result.status, 0, result.stderr);
      assert.equal(readFileSync(path, 'utf8'), base);
    }
    const result = run();
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(path, 'utf8'), base);

    const sequenceFirst = 'Installers:\n- InstallerLocale: en-US\n  Architecture: x64\nManifestType: installer\n';
    writeFileSync(path, sequenceFirst);
    assert.equal(run().status, 0);
    assert.equal(readFileSync(path, 'utf8'), 'Installers:\n-\n  Architecture: x64\nManifestType: installer\n');

    const locale = 'PackageLocale: en-US\nManifestType: defaultLocale\n';
    writeFileSync(path, locale);
    assert.notEqual(run().status, 0);
    assert.equal(readFileSync(path, 'utf8'), locale);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
