#!/usr/bin/env bash
# Revives the tracking-model orchestrator after a Claude usage-limit cutoff.
# Watches the orchestrator transcript; when its last entry is a limit error it
# probes cheaply until the limit has reset, then resumes the session in the
# background. Exits when the work item is archived. Logs to ~/.cache/salah-agents.
set -u
REPO=/home/saidburxon/rust/salah
WORK=$REPO/docs/work/tracking-model
PROJ=${CLAUDE_CONFIG_DIR:?}/projects/-home-saidburxon-rust-salah
SID=${1:-1e3cb88a-5a7d-45ca-8eac-21766d5044fd}
LOG=$HOME/.cache/salah-agents/revive.log
mkdir -p "${LOG%/*}"
PROMPT="You were cut off by the Claude usage limit and revived by docs/work/tracking-model/revive.sh. Continue as orchestrator of tracking-model: read docs/work/tracking-model/state.md (and w1-progress.md if present), check git, 'claude agents --json', and Codex limits in ~/.codex/sessions, dispatch pending briefs, verify finished slices, and update state.md. Spend usage economically."

log() { printf '%s %s\n' "$(date '+%F %T')" "$*" >>"$LOG"; }

limited() {
  local t=$PROJ/$SID.jsonl
  [ -f "$t" ] || return 1
  tail -n 6 "$t" | grep '"isApiErrorMessage":true' | grep -qiE 'limit|resets'
}

probe_ok() {
  local out
  out=$(cd "$REPO" && timeout 180 claude -p --model haiku "Reply with the word OK" 2>&1)
  log "probe: ${out:0:160}"
  ! grep -qiE 'limit|resets|error' <<<"$out"
}

log "watching $SID"
while [ -d "$WORK" ]; do
  if limited; then
    if probe_ok; then
      out=$(cd "$REPO" && claude --bg --resume "$SID" -n salah-fb-r --permission-mode auto "$PROMPT" 2>&1)
      log "revived: ${out:0:200}"
      sleep 30
      new=$(claude agents --json 2>/dev/null | jq -r '.[] | select(.name=="salah-fb-r") | .sessionId' | tail -n1)
      [ -n "$new" ] && SID=$new && log "now watching $SID"
      sleep 600
    else
      sleep 300
    fi
  else
    sleep 120
  fi
done
log "work item archived; exiting"
