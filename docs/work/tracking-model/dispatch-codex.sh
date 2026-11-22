#!/usr/bin/env bash
# Starts a Codex worker on a brief as a user service that outlives the caller.
# Usage: dispatch-codex.sh <brief.md> <tag>   e.g. dispatch-codex.sh w1-brief.md w1
# Output: ~/.cache/salah-agents/<tag>.log (JSON events) and <tag>-last.md (final message).
set -eu
REPO=/home/saidburxon/rust/salah
DIR=$REPO/docs/work/tracking-model
brief=$DIR/${1:?brief}
tag=${2:?tag}
out=$HOME/.cache/salah-agents
mkdir -p "$out"
[ -f "$brief" ] || { echo "no brief $brief" >&2; exit 1; }
systemctl --user is-active --quiet "salah-codex-$tag" && { echo "salah-codex-$tag already running" >&2; exit 1; }
systemd-run --user --unit="salah-codex-$tag" --collect \
  --setenv=PATH="$PATH" --setenv=HOME="$HOME" --working-directory="$REPO" \
  bash -c "codex exec -C '$REPO' -s workspace-write --json -o '$out/$tag-last.md' \"\$(cat '$brief')\" >'$out/$tag.log' 2>&1"
echo "started salah-codex-$tag; log $out/$tag.log"
