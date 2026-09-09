const { test } = require('node:test');
const assert = require('node:assert/strict');
const checkWingetAccess = require('./check-winget-access.cjs');

function fixture({ scopes = 'public_repo', fork = {}, authError = false, repoError = false } = {}) {
  const calls = [];
  return {
    calls,
    token: 'test-token',
    github: { rest: {
      users: { getAuthenticated: async () => {
        calls.push('user');
        if (authError) throw new Error('sensitive upstream error');
        return { headers: { 'x-oauth-scopes': scopes } };
      } },
      repos: { get: async (params) => {
        calls.push(params);
        if (repoError) throw new Error('sensitive upstream error');
        return { data: {
          fork: true,
          parent: { full_name: 'microsoft/winget-pkgs' },
          permissions: { push: true },
          ...fork,
        } };
      } },
    } },
  };
}

test('rejects missing credentials without making API calls', async () => {
  const input = fixture();
  await assert.rejects(checkWingetAccess({ ...input, token: '' }), /WINGET_TOKEN is missing/);
  assert.deepEqual(input.calls, []);
});

test('rejects insufficient classic scopes and fine-grained tokens', async () => {
  for (const scopes of ['', 'read:user', 'repo:status']) {
    const input = fixture({ scopes });
    await assert.rejects(checkWingetAccess(input), /classic PAT with public_repo/);
    assert.deepEqual(input.calls, ['user']);
  }
});

test('rejects the reported branch-creation permission failure', async () => {
  await assert.rejects(checkWingetAccess(fixture({ fork: { permissions: { push: false } } })), /cannot write/);
});

test('rejects unavailable or unrelated forks', async () => {
  for (const fork of [{ archived: true }, { disabled: true }, { permissions: {} },
    { fork: false }, { parent: { full_name: 'another/repo' } }]) {
    await assert.rejects(checkWingetAccess(fixture({ fork })));
  }
});

test('accepts public_repo and the broader repo scope for a writable fork', async () => {
  for (const scopes of ['read:user, public_repo', 'repo']) {
    const input = fixture({ scopes });
    await checkWingetAccess(input);
    assert.deepEqual(input.calls, ['user', { owner: 'PsyChonek', repo: 'winget-pkgs' }]);
  }
});

test('API failures give actionable messages without forwarding upstream errors', async () => {
  for (const options of [{ authError: true }, { repoError: true }]) {
    await assert.rejects(checkWingetAccess(fixture(options)), error => {
      assert.match(error.message, /Update WINGET_TOKEN in the release environment/);
      assert.doesNotMatch(error.message, /sensitive upstream error/);
      return true;
    });
  }
});
