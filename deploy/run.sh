#!/bin/bash

cargo install --locked watchexec-cli

cd /data/git-sync/overlay-mcp.git && watchexec \
    --exts rs \
    --poll 5s \
    -- 'cd /data/git-sync/overlay-mcp.git && cargo run --bin overlay-mcp -- --config /data/overlay-mcp/config.json'