This wrapper requires local BMALPH assets under `_bmad/`, which this repository intentionally keeps out of git. If `_bmad/` is missing in a fresh clone or workspace, run `make bmalph-setup` first, or `bmalph upgrade --force` if BMALPH is already installed for this repo.

# Project Status

Show the current bmalph project status, including phase, Ralph progress, and version info.

## How to Run

Execute the CLI command:
    bmalph status

## What It Does

- Shows current phase (1-4) and status (planning, implementing, completed)
- Displays Ralph loop progress (tasks completed / total)
- Reports installed bmalph version and bundled asset versions
- Indicates whether updates are available
