#!/bin/bash

TARGET_DIR="./"  # ← ここを対象ディレクトリに変更

find "$TARGET_DIR" -type f -exec sed -i '' 's/:REJECT/:WARN/g' {} +
