#!/usr/bin/env bash
# Starts (or resumes) a Codex worker as a user service that outlives the caller.
# Usage: dispatch-codex.sh <brief.md> <tag>            new run from a brief
#        dispatch-codex.sh --resume <tag> "<message>"  continue that worker's thread
# Output: ~/.cache/salah-agents/<tag>.log (JSON events, appended) and <tag>-last.md.
set -eu
REPO=/home/saidburxon/rust/salah
DIR=$REPO/docs/work/tracking-model
out=$HOME/.cache/salah-agents
mkdir -p "$out"
if [ "${1:-}" = --resume ]; then
  tag=${2:?tag}; msg=${3:?message}
  tid=$(grep -m1 -o '"thread_id":"[^"]*"' "$out/$tag.log" | cut -d'"' -f4)
  [ -n "$tid" ] || { echo "no thread id for $tag" >&2; exit 1; }
  printf '%s' "$msg" >"$out/$tag-msg.txt"
  cmd="codex exec --json -o '$out/$tag-last.md' -s workspace-write resume '$tid' - <'$out/$tag-msg.txt' >>'$out/$tag.log' 2>&1"
else
  brief=$DIR/${1:?brief}; tag=${2:?tag}
  [ -f "$brief" ] || { echo "no brief $brief" >&2; exit 1; }
  cmd="codex exec -C '$REPO' -s workspace-write --json -o '$out/$tag-last.md' - <'$brief' >'$out/$tag.log' 2>&1"
fi
systemctl --user is-active --quiet "salah-codex-$tag" && { echo "salah-codex-$tag already running" >&2; exit 1; }
systemctl --user reset-failed "salah-codex-$tag" 2>/dev/null || true
systemd-run --user --unit="salah-codex-$tag" --collect \
  --setenv=PATH="$PATH" --setenv=HOME="$HOME" --working-directory="$REPO" bash -c "$cmd"
echo "started salah-codex-$tag"
