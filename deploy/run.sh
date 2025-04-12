#!/bin/bash

cd /data/git-sync/overlay-mcp.git

cargo install --locked watchexec-cli

watchexec \
    --exts rs \
    --poll 5s \
    -- 'cd /data/git-sync/overlay-mcp.git && cargo run --bin overlay-mcp -- --config /data/overlay-mcp/config.json'