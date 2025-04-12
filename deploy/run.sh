#!/bin/bash

cd /data/git-sync/overlay-mcp.git

cargo install --locked watchexec-cli

watchexec \
    --exts rs \
    -- 'cargo run --release --bin overlay-mcp -- --config /data/overlay-mcp/config.json'