#!/bin/bash

curl -fsSL https://apt.cli.rs/pubkey.asc | tee -a /usr/share/keyrings/rust-tools.asc
curl -fsSL https://apt.cli.rs/rust-tools.list | tee /etc/apt/sources.list.d/rust-tools.list
apt update
apt install watchexec-cli

watchexec \
    --exts rs \
    --poll 5s \
    -w /data/git-sync/overlay-mcp.git \
    -- 'cd /data/git-sync/overlay-mcp.git && cargo run --bin overlay-mcp -- --config /data/overlay-mcp/config.json'