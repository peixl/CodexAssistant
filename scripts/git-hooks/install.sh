#!/usr/bin/env bash
# Enable the tracked git hooks in scripts/git-hooks for this clone.
# Run once after cloning: `bash scripts/git-hooks/install.sh`
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

git config --local core.hooksPath scripts/git-hooks

find scripts/git-hooks -type f \
  \( -name 'pre-*' -o -name 'post-*' -o -name 'commit-*' -o -name 'prepare-*' \) \
  -exec chmod +x {} +

echo "git hooks enabled at scripts/git-hooks"
echo "(run 'git config --local --unset core.hooksPath' to disable)"
