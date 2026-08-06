#!/usr/bin/env bash

SESSION="fgscsv"

if tmux has-session -t "$SESSION" 2>/dev/null; then
  if [ -t 1 ]; then
    exec tmux attach -t "$SESSION"
  fi
  exit 0
fi

PROJECT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"

tmux new-session -d \
  -s "$SESSION"     \
  -c "$PROJECT_DIR"

tmux select-window -t "$SESSION":1

if [ -t 1 ]; then
  exec tmux attach -t "$SESSION"
fi
