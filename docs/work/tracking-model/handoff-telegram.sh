#!/usr/bin/env bash
# After tracking-model is finished: resume the telegram-vim-rust lead (session
# 7fca2a10) in the background to carry out its task list 1-4.
set -eu
cd /home/saidburxon/go/src/telegram-focus/telegram-vim-rust
claude --bg --resume 7fca2a10-8233-4083-b0b7-010b1983feec -n telegram-orch --permission-mode auto \
"Orchestrator handoff from salah-orch (the user asked for this). Continue as lead and orchestrator for this repo: carry out your proposed task list items 1-4 in your suggested order: 4 (split the files over 800 lines so scripts/check.sh passes, commit the presence/topics work), then 1 (sending media inside topics), then 2 (topic correctness) and 3 (performance). Use workers as docs/START.md describes, verify each step, commit at green checkpoints, keep docs/START.md and the topic state current. Claude and Codex usage limits are shared; work economically. Items 5-6 still need the user."
