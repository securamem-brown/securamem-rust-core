# SecuraMem – User Guide

Quick reference for this project. Data lives under .securamem/. Legacy .antigoldfishmode/ is still read for compatibility.

## First steps
- smem status
- smem vector-status
- smem health [--since 7]

## Index & watch
- smem index-code --symbols --path .
- smem watch-code --path src --symbols --max-chunk 200

Policy tips:
- smem policy allow-command watch-code
- smem policy allow-path "**/*"

## Search
- smem search-code "query" --hybrid --preview 3

## Maintenance
- smem digest-cache --list --limit 20
- smem reindex-file <file> [--symbols]
- smem reindex-folder <folder> [--symbols]
- smem gc --prune-vectors --drop-stale-digests --vacuum

## Air-gapped export
- smem export-context --out ./.securamem/ctx.smemctx --type code [--sign]
- smem import-context ./.securamem/ctx.smemctx

Receipts: .securamem/receipts/*.json
Journal: .securamem/journal.jsonl
Policy: .securamem/policy.json
