#!/usr/bin/env bash
# Release flow of release.yml, step 6 (architecture-delivery.md "CI/CD Workflows", DD-11, PF-62):
# version from conventional commits, changelog and manifest through cocogitto, commit
# `chore(release): vX.Y.Z [skip ci]`, tag, push and GitHub release.
#
# Usage: scripts/release.sh [--version <x.y.z>]
# Needs: cog 7.0.0, git, gh (GH_TOKEN in the environment), an `origin` remote and the branch main.
# Exit status: 0 released, or nothing to release; 1 failure.
set -euo pipefail

usage() {
  echo "usage: scripts/release.sh [--version <x.y.z>]" >&2
}

explicit=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      if [[ $# -lt 2 ]]; then
        usage
        exit 1
      fi
      explicit="$2"
      shift 2
      ;;
    --version=*)
      explicit="${1#--version=}"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 1
      ;;
  esac
done

if [[ -n "${explicit}" && ! "${explicit}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "release: the version must be x.y.z, got: ${explicit}" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$(dirname "${script_dir}")"

# Prints "<id> <true|false>" for every commit of the range of the bump that cocogitto reads as conventional, as
# `cog changelog` lists them with a template (true when cocogitto marks the commit breaking; a breaking commit is
# listed whatever its type). The range is <latest tag>..HEAD, or the whole history of HEAD when there is no
# vX.Y.Z tag: cocogitto then reads the oldest commit too, so git must list it. Merge commits are dropped as
# `cog bump` drops them.
range_commits() {
  local latest="$1" cog_range git_range template listed
  if [[ -n "${latest}" ]]; then
    cog_range="${latest}..HEAD"
    git_range="${latest}..HEAD"
  else
    cog_range="$(git rev-list HEAD | tail -n 1)..HEAD"
    git_range=HEAD
  fi
  [[ -n "$(git rev-list --no-merges "${git_range}")" ]] || return 0
  template="$(mktemp "${TMPDIR:-/tmp}/release-commits.XXXXXX")"
  printf '%s\n' '{% for commit in commits %}{{ commit.id }} {{ commit.breaking_change }}' '{% endfor %}' >"${template}"
  if ! listed="$(cog changelog "${cog_range}" --template "${template}")"; then
    rm -f "${template}"
    echo "release: cocogitto could not read the commits of ${cog_range}" >&2
    return 1
  fi
  rm -f "${template}"
  printf '%s\n' "${listed}" | grep -F -f <(git rev-list --no-merges "${git_range}") || true
}

# Succeeds when commit <id> carries a breaking change as the conventional-commit definition states it (FR98(a)): a
# `!` before the colon of the subject, or the footer `BREAKING CHANGE: <value>` (or `BREAKING-CHANGE: <value>`).
# Footers follow the body after a blank line: the footer section is the run of paragraphs at the end of the message,
# after the subject paragraph, that each start with a footer (a token of letters, digits and `-`, or
# `BREAKING CHANGE`, then `: ` or ` #`), so trailers appended in a paragraph of their own do not hide it. The
# breaking footer is a line of that section that starts with the upper-case token, a colon, a blank and a value.
# The same line in a paragraph of the body, indented (a continuation or an example), inside a sentence, in lower
# case or without the blank is no breaking change, although cocogitto 7.0.0 marks a commit breaking for any line of
# the body that starts with the token.
breaking_change() {
  local subject
  subject="$(git log -1 --format=%s "$1")"
  if [[ "${subject%%:*}" == *'!' ]]; then
    return 0
  fi
  git log -1 --format=%B "$1" | sed 's/[[:space:]]*$//' | awk '
    BEGIN { RS = "" }
    { paragraph[NR] = $0 }
    END {
      for (i = NR; i > 1 && paragraph[i] ~ /^(BREAKING CHANGE|[A-Za-z0-9-]+)(: | #)/; i--)
        if (("\n" paragraph[i]) ~ /\nBREAKING[ -]CHANGE: [^[:space:]]/) found = 1
      exit !found
    }'
}

# Prints the bump that the fix and feat commits among the listed ones ask for: minor, patch, or nothing.
plain_level() {
  local id flag type level=""
  while read -r id flag; do
    [[ -n "${id}" ]] || continue
    type="$(git log -1 --format=%s "${id}")"
    type="$(printf '%s' "${type%%[(!:]*}" | tr '[:upper:]' '[:lower:]')"
    case "${type}" in
      feat) level="minor" ;;
      fix) [[ -n "${level}" ]] || level="patch" ;;
    esac
  done <<<"$1"
  printf '%s\n' "${level}"
}

if [[ -n "${explicit}" ]]; then
  bump=(--version "${explicit}")
else
  # 1. Is there anything to release? cocogitto prints a sentence and exits 0 when nothing bumps. cocogitto 7.0.0
  # ignores a breaking change below 1.0.0 (a fix stays a patch, a docs or refactor commit releases nothing) and from
  # 1.0.0 on counts more lines than the footer section, so the script decides on breaking changes itself (FR98(a),
  # DD-11) among the commits that cocogitto reads in the range of `cog bump --auto`.
  next="$(cog bump --auto --dry-run)" || {
    echo "${next}" >&2
    exit 1
  }
  latest="$(git tag --list 'v*' --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | head -n 1 || true)"
  commits="$(range_commits "${latest}")"
  breaking=""
  marked=""
  while read -r id flag; do
    [[ "${flag}" == true ]] || continue
    if breaking_change "${id}"; then
      breaking="${id}"
    else
      marked="${id}"
    fi
  done <<<"${commits}"
  # A prerelease tag (v0.1.0-rc.1) is no release: without a stable tag the next release is the first one.
  if [[ -z "${latest}" ]]; then
    # cocogitto reads no breaking change at 0.0.0 either, so its dry run answers for the fix and feat commits.
    if [[ ! "${next}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ && -z "${breaking}" ]]; then
      echo "${next}"
      exit 0
    fi
    # The first release is the manifest version, whatever the first releasable commit is (DD-11).
    manifest_version="$(awk '
      /^\[/ { in_table = ($0 == "[workspace.package]") }
      in_table && /^version[[:space:]]*=/ { gsub(/^[^"]*"|".*$/, ""); print; exit }
    ' Cargo.toml)"
    if [[ ! "${manifest_version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "release: cannot read [workspace.package].version from Cargo.toml" >&2
      exit 1
    fi
    bump=(--version "${manifest_version}")
  elif [[ -n "${breaking}" && "${latest}" == v0.* ]]; then
    # While the version is 0.y.z a breaking change raises the minor number (DD-11).
    bump=(--minor)
  elif [[ -n "${breaking}" ]]; then
    bump=(--major)
  elif [[ -n "${marked}" && "${latest}" != v0.* ]]; then
    # From 1.0.0 on `--auto` would make a major bump for a commit that cocogitto alone marks breaking.
    case "$(plain_level "${commits}")" in
      minor) bump=(--minor) ;;
      patch) bump=(--patch) ;;
      *)
        echo "release: nothing to release since ${latest}: no fix, no feat and no breaking-change footer" \
          "(a BREAKING CHANGE line outside the footer section does not count)"
        exit 0
        ;;
    esac
  elif [[ "${next}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    # Below 1.0.0 cocogitto ignores breaking changes, and from 1.0.0 on it marks none here: `--auto` is the fix and
    # feat bump.
    bump=(--auto)
  else
    echo "${next}"
    exit 0
  fi
fi

# 2. Bump: pre-bump hook, changelog, commit and tag.
cog bump "${bump[@]}" --skip-ci

tag="$(git tag --points-at HEAD --list 'v*' | head -n 1)"
if [[ -z "${tag}" ]]; then
  echo "release: cocogitto did not tag HEAD" >&2
  exit 1
fi

# 3. cocogitto names its commit chore(version): ...; amend it and move the tag.
git commit --amend --no-verify -m "chore(release): ${tag} [skip ci]"
git tag -f "${tag}"

# 4. Push. A non-fast-forward rejection fails the job; the next push to main releases both changes.
git push origin HEAD:main
git push origin "${tag}"

# 5. GitHub release.
mkdir -p target
cog changelog --at "${tag}" >target/release-notes.md
gh release create "${tag}" --title "${tag}" --notes-file target/release-notes.md --verify-tag
