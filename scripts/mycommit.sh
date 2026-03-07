#!/usr/bin/env bash

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

TMP_DIR="tmp"
mkdir -p "$TMP_DIR"

echo "Saving staged changes to $TMP_DIR/staged.diff..."
git diff --staged > "$TMP_DIR/staged.diff"

echo "Gathering commit history to $TMP_DIR/commits.log..."
git log > "$TMP_DIR/commits.log"

echo "Generating directory structure to $TMP_DIR/tree.log..."
echo "$(basename "$PROJECT_ROOT")" > "$TMP_DIR/tree.log"
tree -a --dirsfirst -I 'node_modules|.git|.venv' \
  | sed '1d' >> "$TMP_DIR/tree.log"

echo "All information has been saved to $PROJECT_ROOT/$TMP_DIR."