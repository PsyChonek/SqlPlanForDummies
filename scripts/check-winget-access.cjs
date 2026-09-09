// Read-only checks using the same token as winget-releaser.
module.exports = async function checkWingetAccess({ github, token }) {
  const setup = 'Update WINGET_TOKEN in the release environment with a classic PAT with public_repo scope and write access to PsyChonek/winget-pkgs.';
  if (!token) {
    throw new Error(`WINGET_TOKEN is missing. ${setup}`);
  }

  let user;
  try {
    user = await github.rest.users.getAuthenticated();
  } catch {
    throw new Error(`Cannot authenticate WINGET_TOKEN with GitHub. Check token expiry and GitHub availability. ${setup}`);
  }

  const scopes = (user.headers['x-oauth-scopes'] || '').split(',').map(scope => scope.trim());
  if (!scopes.includes('public_repo') && !scopes.includes('repo')) {
    throw new Error(`WINGET_TOKEN must be a classic PAT with public_repo scope; fine-grained PATs are not supported by winget-releaser. ${setup}`);
  }

  let fork;
  try {
    ({ data: fork } = await github.rest.repos.get({ owner: 'PsyChonek', repo: 'winget-pkgs' }));
  } catch {
    throw new Error(`Cannot access PsyChonek/winget-pkgs with WINGET_TOKEN. Check that the fork exists and GitHub is available. ${setup}`);
  }
  if (!fork.fork || fork.parent?.full_name?.toLowerCase() !== 'microsoft/winget-pkgs') {
    throw new Error('PsyChonek/winget-pkgs must be a fork of microsoft/winget-pkgs.');
  }
  if (fork.archived || fork.disabled || !fork.permissions?.push) {
    throw new Error(`WINGET_TOKEN cannot write to PsyChonek/winget-pkgs, or the fork is archived/disabled. ${setup}`);
  }
};
