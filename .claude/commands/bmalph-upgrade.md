This wrapper requires local BMALPH assets under `_bmad/`, which this repository intentionally keeps out of git. If `_bmad/` is missing in a fresh clone or workspace, run `make bmalph-setup` first, or `bmalph upgrade --force` if BMALPH is already installed for this repo.

# Upgrade Bundled Assets

Update BMAD and Ralph assets to match the currently installed bmalph version.

## How to Run

Execute the CLI command:
    bmalph upgrade

## What It Does

- Overwrites `_bmad/` with the latest bundled BMAD agents and workflows
- Overwrites `.ralph/` base files with the latest bundled Ralph loop and libraries
- Preserves project-specific state (config, fix plan progress, specs)
- Reports which files were updated
