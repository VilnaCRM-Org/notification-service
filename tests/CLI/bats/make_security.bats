#!/usr/bin/env bats
# Tests of the security gates and of the hardening claims (architecture-delivery.md "CLI Tests (Bats)", file
# make_security.bats, DD-21, "Containers and Compose"): gates G9 and G10, the inspection of the production image,
# the inspection of the rendered production reference (NFR12, FR91) and of the database readiness probe (NFR33).
# JSON is read with json_query of the helper (the pinned Node image), so the host needs no jq.
#
# Passing runs of the two gates are not repeated (DD-02). The key pair of G9 is generated at test time and is
# never stored. The image under test is the production image of the environment (APP_IMAGE); it is built before
# the group runs, so these tests only read and run it.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=load

# Upper bound of the production image: 100 MiB.
IMAGE_SIZE_LIMIT=104857600

setup() {
  _common_setup
}

teardown() {
  restore_planted
  if [[ -n "${container:-}" ]]; then
    docker rm --force "${container}" >/dev/null 2>&1 || true
  fi
  rm -rf -- target/reports/image-scan
}

# Value of a Make variable after the Makefile was read, so the tests follow the caller's environment.
make_var() {
  make --no-print-directory --silent -f Makefile -f - "print-$1" <<'MAKEFILE'
.RECIPEPREFIX = >
print-%:
> @printf '%s' "$($*)"
MAKEFILE
}

# The production image of the environment; the test fails when it does not exist.
production_image() {
  local image
  image="$(make_var APP_IMAGE)"
  docker image inspect "${image}" >/dev/null 2>&1 || fail "the production image ${image} does not exist"
  printf '%s' "${image}"
}

# The entries of a `tar -t` listing (stdin) that NFR12 forbids in the production image, one line each, in input order:
#   shell: sh, ash, bash, dash, ksh, mksh, zsh, busybox or toybox as the name of a file or link in any directory (for
#     example bin/sh, busybox/sh of the distroless debug images, or a copy in usr/local/bin); `tar -t` writes a
#     directory with a trailing slash, and a directory of that name (usr/share/zsh/ holds completion files) is no shell;
#   package manager or compiler: apt, apt-get, dpkg, apk, cc, gcc, c++, rustc, cargo or rustup in a directory named
#     bin or sbin (bin, sbin, usr/bin, usr/sbin, usr/local/bin, and the tool directories of a Rust image such as
#     usr/local/cargo/bin);
#   not the binary: anything under app/ other than app/service;
#   test file: Rust source (.rs), a path through a directory named tests or features, or a Makefile.
# The awk program uses only what mawk and busybox awk also support.
image_content_findings() {
  awk '
    BEGIN {
      count = split("sh ash bash dash ksh mksh zsh busybox toybox", names, " ")
      for (i = 1; i <= count; i++) shell[names[i]] = 1
      count = split("apt apt-get dpkg apk cc gcc c++ rustc cargo rustup", names, " ")
      for (i = 1; i <= count; i++) tool[names[i]] = 1
    }
    {
      entry = $0
      path = entry
      sub(/^\.\//, "", path)
      directory = sub(/\/$/, "", path)
      if (path == "" || path == ".") next
      depth = split(path, part, "/")
      base = part[depth]
      parent = (depth > 1) ? part[depth - 1] : ""
    }
    !directory && (base in shell) { print "shell: " entry; next }
    (parent == "bin" || parent == "sbin") && (base in tool) { print "package manager or compiler: " entry; next }
    path ~ /^app\// && path != "app/service" { print "not the binary: " entry; next }
    path ~ /\.rs$/ || ("/" path "/") ~ /\/(tests|features)\// || base == "Makefile" { print "test file: " entry; next }
  '
}

# `docker compose config` of the production reference with the given variables set and no .env file read.
render_production_reference() {
  env -u DATABASE_URL -u AWS_REGION "$@" docker compose --env-file /dev/null \
    -f docker-compose.yml -f docker-compose.prod.yml config --format json
}

# --- G9: secrets scan --------------------------------------------------------------------------------------

@test "G9: a private key generated at test time in an untracked file makes secrets-scan report leaks found" {
  require_docker
  require_command openssl
  openssl genrsa -out "${BATS_TEST_TMPDIR}/key.pem" 2048 2>/dev/null
  plant_new planted_private_key.pem "${BATS_TEST_TMPDIR}/key.pem"
  run make --no-print-directory secrets-scan
  assert_failure
  assert_output --partial 'leaks found'
  assert_output --partial 'planted_private_key.pem:private-key'
}

# A throw-away repository with the scanner script, and a stand-in for docker that lists the copy of the working files
# which the script mounts at /scan (and fails the scan of it when STUB_LEAK is set). `tar` is replaced by a stand-in
# that fails: the copy must not depend on a tar option that only GNU tar has (macOS ships bsdtar).
secrets_scan_fixture() {
  local repo="${BATS_TEST_TMPDIR}/repo" bin="${BATS_TEST_TMPDIR}/bin"
  mkdir -p "${repo}/scripts" "${repo}/dir" "${bin}"
  cp scripts/secrets-scan.sh "${repo}/scripts/"
  git -C "${repo}" init --quiet
  printf 'ignored.txt\n' >"${repo}/.gitignore"
  : >"${repo}/.gitleaks.toml"
  : >"${repo}/.gitleaksignore"
  printf 'one\n' >"${repo}/a.txt"
  printf 'two\n' >"${repo}/dir/b.txt"
  printf 'three\n' >"${repo}/e f.txt"
  git -C "${repo}" add -- .
  # Listed by git, gone from the working tree.
  rm -- "${repo}/dir/b.txt"
  printf 'four\n' >"${repo}/c.txt"
  printf 'five\n' >"${repo}/ignored.txt"
  ln -s a.txt "${repo}/link.txt"
  cat >"${bin}/docker" <<'STUB'
#!/bin/bash
[[ "$1" == run ]] || exit 0
for argument in "$@"; do
  if [[ "${argument}" == *:/scan:ro ]]; then
    (cd "${argument%:/scan:ro}" && find . \( -type f -o -type l \) | LC_ALL=C sort) >"${LISTING}"
    [[ -z "${STUB_LEAK:-}" ]] || exit 1
  fi
done
exit 0
STUB
  printf '#!/bin/sh\necho "tar was called" >&2\nexit 99\n' >"${bin}/tar"
  chmod +x "${bin}/docker" "${bin}/tar"
}

@test "G9: the copy of the working files skips a listed file that is gone and does not use tar" {
  secrets_scan_fixture
  LISTING="${BATS_TEST_TMPDIR}/listing" GITLEAKS_IMAGE=stub DOCKER="${BATS_TEST_TMPDIR}/bin/docker" PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" \
    run "${BATS_TEST_TMPDIR}/repo/scripts/secrets-scan.sh"
  assert_success
  assert_output --partial 'secrets-scan: no leaks found'
  refute_output --partial 'tar was called'
  assert_equal "$(cat "${BATS_TEST_TMPDIR}/listing")" "$(printf '%s\n' './.gitignore' './.gitleaks.toml' './.gitleaksignore' './a.txt' './c.txt' './e f.txt' './link.txt' './scripts/secrets-scan.sh')"
}

@test "G9: a finding of the scanner exits 1, a temporary directory that cannot be created exits 2" {
  secrets_scan_fixture
  LISTING="${BATS_TEST_TMPDIR}/listing" STUB_LEAK=1 GITLEAKS_IMAGE=stub DOCKER="${BATS_TEST_TMPDIR}/bin/docker" PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" \
    run "${BATS_TEST_TMPDIR}/repo/scripts/secrets-scan.sh"
  assert_failure 1
  assert_output --partial 'secrets-scan: leaks found'
  LISTING="${BATS_TEST_TMPDIR}/listing" TMPDIR="${BATS_TEST_TMPDIR}/missing" GITLEAKS_IMAGE=stub DOCKER="${BATS_TEST_TMPDIR}/bin/docker" \
    run "${BATS_TEST_TMPDIR}/repo/scripts/secrets-scan.sh"
  assert_failure 2
  assert_output --partial 'could not create a temporary directory'
}

@test "G9: a listed file that cannot be copied exits 2 with the message and does not start the scan of the copy" {
  secrets_scan_fixture
  # A stand-in for cp that fails (a file the caller cannot read would not fail for root).
  printf '#!/bin/sh\necho "cp stand-in failed" >&2\nexit 1\n' >"${BATS_TEST_TMPDIR}/bin/cp"
  chmod +x "${BATS_TEST_TMPDIR}/bin/cp"
  LISTING="${BATS_TEST_TMPDIR}/listing" GITLEAKS_IMAGE=stub DOCKER="${BATS_TEST_TMPDIR}/bin/docker" PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" \
    run "${BATS_TEST_TMPDIR}/repo/scripts/secrets-scan.sh"
  assert_failure 2
  assert_output --partial 'cp stand-in failed'
  assert_output --partial 'secrets-scan: could not copy the working files'
  refute_output --partial 'no leaks found'
  assert_file_not_exists "${BATS_TEST_TMPDIR}/listing"
}

@test "G9: a key that exists only in the history makes secrets-scan report leaks found" {
  require_docker
  require_command openssl
  local repo="${BATS_TEST_TMPDIR}/history" image
  image="$(pinned_value GITLEAKS_IMAGE)"
  mkdir -p "${repo}/scripts"
  cp scripts/secrets-scan.sh "${repo}/scripts/"
  cp .gitleaks.toml "${repo}/"
  : >"${repo}/.gitleaksignore"
  # Git of the throw-away repository reads no configuration of the caller: a global signing setting, commit hook
  # or ignore file (core.excludesFile, or the default ignore file of XDG_CONFIG_HOME) would make a step fail.
  repo_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${repo}" -c user.name=bats \
      -c user.email=bats@example.invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null \
      -c core.excludesFile=/dev/null "$@"
  }
  repo_git init --quiet
  repo_git add -- .
  repo_git commit --quiet -m 'scanner'
  # The key is generated at test time, committed, and removed again in the next commit: only the history has it.
  openssl genrsa -out "${repo}/history_key.pem" 2048 2>/dev/null
  repo_git add -- history_key.pem
  repo_git commit --quiet -m 'add a key'
  repo_git rm --quiet -- history_key.pem
  repo_git commit --quiet -m 'remove the key'
  assert_file_not_exists "${repo}/history_key.pem"
  GITLEAKS_IMAGE="${image}" run "${repo}/scripts/secrets-scan.sh"
  assert_failure 1
  assert_output --partial 'secrets-scan: leaks found'
  assert_output --partial 'history_key.pem'
  # The finding comes from the history scan: the scan of the working files that follows it reports nothing.
  local working_part
  working_part="$(sed -n '/^secrets-scan: scanning the working files$/,$p' <<<"${output}")"
  [[ -n "${working_part}" ]] || fail "the scan of the working files did not run"
  run grep -c -F 'history_key.pem' <<<"${working_part}"
  assert_output '0'
  run grep -c -F 'no leaks found' <<<"${working_part}"
  refute_output '0'
}

# --- G10: image scan -----------------------------------------------------------------------------------------

# bats test_tags=slow
@test "G10: the vulnerable fixture image makes image-scan exit non-zero with a CRITICAL finding" {
  require_docker
  local fixture
  fixture="$(pinned_value VULNERABLE_FIXTURE_IMAGE)"
  [[ -n "${fixture}" ]] || fail "VULNERABLE_FIXTURE_IMAGE is not pinned in make/tool-versions.mk"
  docker image inspect "${fixture}" >/dev/null 2>&1 || docker pull --quiet "${fixture}" >/dev/null
  IMAGE_TO_SCAN="${fixture}" run make --no-print-directory image-scan
  assert_failure
  assert_output --partial 'CRITICAL'
}

# --- production image ------------------------------------------------------------------------------------------

@test "the production image runs as user 65532" {
  require_docker
  local image
  image="$(production_image)"
  run docker image inspect --format '{{.Config.User}}' "${image}"
  assert_success
  assert_output '65532:65532'
}

@test "the production image has no /bin/sh" {
  require_docker
  local image
  image="$(production_image)"
  run -127 docker run --rm --entrypoint /bin/sh "${image}" -c true
  assert_failure
  assert_output --partial '/bin/sh'
}

@test "the production image holds no shell, package manager, compiler or test file" {
  require_docker
  local image listing="${BATS_TEST_TMPDIR}/listing"
  image="$(production_image)"
  container="$(docker create "${image}")"
  docker export "${container}" | tar -tf - >"${listing}"
  # The export reached tar: the binary is listed.
  run grep -c -x -F 'app/service' "${listing}"
  assert_output '1'
  run image_content_findings <"${listing}"
  assert_success
  assert_output ''
}

@test "the image content check reports each planted shell, package manager, compiler, file under app/ and test file" {
  run image_content_findings <"${BATS_TEST_DIRNAME}/fixtures/image_listing_planted.txt"
  assert_success
  assert_output "$(printf '%s\n' \
    'not the binary: app/src/main.rs' \
    'shell: busybox/busybox' \
    'shell: busybox/sh' \
    'test file: srv/features/api_docs.feature' \
    'test file: srv/tests/' \
    'shell: usr/bin/bash' \
    'package manager or compiler: usr/bin/dpkg' \
    'shell: usr/local/bin/dash' \
    'package manager or compiler: usr/local/cargo/bin/rustc' \
    'test file: usr/src/service/Makefile' \
    'test file: usr/src/service/lib.rs')"
}

# Each of the nine shell names once, as a file or link, at the top level, in a directory named bin or sbin, or in
# another directory; the near misses are a directory named like a shell (busybox/ of the distroless debug images,
# usr/share/zsh/ of completion files) and files whose names only contain a shell name (dot.bashrc is a file of the
# distroless production image).
@test "the image content check reports every shell name in any directory and no near miss" {
  run image_content_findings <<'LISTING'
bin/sh
./bin/ash
usr/bin/bash
usr/local/bin/dash
opt/shells/ksh
usr/lib/mksh
usr/bin/zsh
busybox/
usr/local/sbin/busybox
toybox
usr/share/zsh/
usr/share/base-files/dot.bashrc
usr/bin/bashbug
usr/bin/sha256sum
etc/shells
LISTING
  assert_success
  assert_output "$(printf '%s\n' \
    'shell: bin/sh' \
    'shell: ./bin/ash' \
    'shell: usr/bin/bash' \
    'shell: usr/local/bin/dash' \
    'shell: opt/shells/ksh' \
    'shell: usr/lib/mksh' \
    'shell: usr/bin/zsh' \
    'shell: usr/local/sbin/busybox' \
    'shell: toybox')"
}

@test "the production image is at most 100 MiB" {
  require_docker
  local image size
  image="$(production_image)"
  size="$(docker image inspect --format '{{.Size}}' "${image}")"
  assert [ "${size}" -le "${IMAGE_SIZE_LIMIT}" ]
}

@test "the production image declares the healthcheck of the architecture" {
  require_docker
  local image
  image="$(production_image)"
  run docker image inspect --format '{{json .Config.Healthcheck}}' "${image}"
  assert_success
  run json_query 'JSON.stringify(data.Test) === JSON.stringify(["CMD", "/app/service", "healthcheck"])
    && data.Interval === 10000000000 && data.Timeout === 3000000000 && data.Retries === 3
    && data.StartPeriod <= 30000000000' <<<"${output}"
  assert_success
}

@test "the production image becomes healthy through its own healthcheck with a read-only root and no capabilities" {
  require_docker
  local image status
  image="$(production_image)"
  # The liveness probe does not need the database or LocalStack: the connections are opened lazily.
  container="$(docker run --detach --read-only --cap-drop ALL --security-opt no-new-privileges:true \
    -e APP_ENV=prod -e DATABASE_URL=postgres://app:unused@127.0.0.1:1/app -e AWS_REGION=us-east-1 \
    -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test -e AWS_ENDPOINT_URL_SQS=http://127.0.0.1:1 \
    "${image}")"
  run docker inspect --format '{{json .HostConfig}}' "${container}"
  assert_success
  run json_query 'data.ReadonlyRootfs === true && JSON.stringify(data.CapDrop) === JSON.stringify(["ALL"])
    && data.CapAdd === null && data.SecurityOpt.includes("no-new-privileges:true")' <<<"${output}"
  assert_success
  status=''
  for _ in $(seq 1 40); do
    status="$(docker inspect --format '{{.State.Health.Status}}' "${container}")"
    [[ "${status}" == healthy || "${status}" == unhealthy ]] && break
    sleep 2
  done
  if [[ "${status}" != healthy ]]; then
    docker logs "${container}" >&2 || true
  fi
  assert_equal "${status}" healthy
}

# --- production reference ----------------------------------------------------------------------------------------

@test "the production reference renders with limits, read-only root, dropped capabilities, no-new-privileges and restart policies" {
  require_docker
  run render_production_reference DATABASE_URL=postgres://app:unused@db.example.invalid:5432/app AWS_REGION=eu-west-1
  assert_success
  local rendered="${output}"
  run json_query 'data.services.app.deploy.resources.limits.cpus === 1
    && data.services.app.deploy.resources.limits.memory === "268435456"
    && data.services.app.deploy.resources.limits.pids === 256' <<<"${rendered}"
  assert_success
  run json_query 'data.services.migrate.deploy.resources.limits.cpus === 0.5
    && data.services.migrate.deploy.resources.limits.memory === "134217728"' <<<"${rendered}"
  assert_success
  # Both services, in one query: read-only root, every capability dropped, no-new-privileges.
  run json_query '["app", "migrate"].every((name) => data.services[name].read_only === true
    && JSON.stringify(data.services[name].cap_drop) === JSON.stringify(["ALL"])
    && JSON.stringify(data.services[name].security_opt) === JSON.stringify(["no-new-privileges:true"]))' <<<"${rendered}"
  assert_success
  run json_query 'data.services.app.environment.APP_ENV === "prod"
    && !("AWS_ENDPOINT_URL_SQS" in data.services.app.environment)' <<<"${rendered}"
  assert_success
  # The restart policies (FR91): the service restarts unless it was stopped and gets 15 s to drain; the one-shot
  # migration never restarts.
  run json_query 'data.services.app.restart === "unless-stopped" && data.services.app.stop_grace_period === "15s"
    && data.services.migrate.restart === "no"' <<<"${rendered}"
  assert_success
}

@test "the database readiness probe uses the configured user and database, at most 10 s apart, with 10 retries" {
  require_docker
  run env DB_USER=bats_probe_user DB_NAME=bats_probe_db docker compose --env-file /dev/null -f docker-compose.yml \
    config --format json
  assert_success
  local rendered="${output}"
  # pg_isready reports success for any role name, so only the rendered command shows that the probe asks for the
  # role and the database of the container, which take the two configured values.
  run json_query 'data.services.database.environment.POSTGRES_USER === "bats_probe_user"
    && data.services.database.environment.POSTGRES_DB === "bats_probe_db"' <<<"${rendered}"
  assert_success
  run json_query 'data.services.database.healthcheck.test[0] === "CMD-SHELL"
    && /^pg_isready -U "[$]+POSTGRES_USER" -d "[$]+POSTGRES_DB"$/.test(data.services.database.healthcheck.test[1])' \
    <<<"${rendered}"
  assert_success
  # Compose prints durations as Go durations (5s, 1m0s); the interval must be at most 10 s.
  run json_query '(() => {
    const match = /^(?:(\d+)m)?(?:(\d+(?:\.\d+)?)s)?$/.exec(data.services.database.healthcheck.interval);
    const seconds = match ? Number(match[1] || 0) * 60 + Number(match[2] || 0) : Infinity;
    return seconds > 0 && seconds <= 10 && data.services.database.healthcheck.retries === 10;
  })()' <<<"${rendered}"
  assert_success
}

@test "the production reference rendered with --env-file /dev/null fails and names each required variable" {
  require_docker
  # Compose stops at the first missing variable and its order is not fixed, so each case leaves exactly one
  # required variable unset.
  run render_production_reference AWS_REGION=eu-west-1
  assert_failure
  assert_output --partial 'DATABASE_URL is required'
  run render_production_reference DATABASE_URL=postgres://app:unused@db.example.invalid:5432/app
  assert_failure
  assert_output --partial 'AWS_REGION is required'
}
