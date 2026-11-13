#!/usr/bin/env bats
# Tests of the database targets (architecture-delivery.md "CLI Tests (Bats)", file make_database.bats):
# `migrate`, `migrate-rollback` and `create-migration`.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=quality

# The tests that apply migrations work on a database of their own on the server of the development compose
# project, created by `throwaway_database` and dropped in `teardown`; `DATABASE_URL` names it, so `make migrate` and `make migrate-rollback` (the service
# binary keeps a variable that is already set) never touch the development database `app`. Container mode forwards
# no `DATABASE_URL` into the container, so these tests are skipped there with a reason.

setup() {
  _common_setup
}

# Creates the throw-away database and points DATABASE_URL at it, or skips the test.
throwaway_database() {
  require_docker
  require_host_mode "the throw-away database is reached through DATABASE_URL, which the container does not receive"
  run make --no-print-directory ensure-database
  assert_success
  bats_db="app_bats_${BASHPID}_${RANDOM}"
  db_user="$(env_value DB_USER)"
  [[ "${bats_db}" != "$(env_value DB_NAME)" ]]
  server_sql "create database ${bats_db}"
  DATABASE_URL="$(scripts/with-env.sh .env -- bash -c "printf '%s' \"postgres://\${DB_USER}:\${DB_PASSWORD}@\${DB_HOST}:\${DB_PORT}/\$1\"" _ "${bats_db}")"
  export DATABASE_URL
}

teardown() {
  restore_planted
  if [[ -n "${bats_db:-}" ]]; then
    server_sql "drop database if exists ${bats_db} with (force)" >/dev/null 2>&1 || true
  fi
}

# Value of a variable of .env as the scripts see it (the caller's value wins).
env_value() {
  scripts/with-env.sh .env -- printenv "$1"
}

# Runs a statement on the maintenance database of the server of the compose project.
server_sql() {
  "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "${db_user}" -d postgres -c "$1"
}

# Runs a statement against the throw-away database.
dev_sql() {
  "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "${db_user}" -d "${bats_db}" -c "$1"
}

# Number of tables in the public schema of a database of the server.
table_count() {
  "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "${db_user}" -d "$1" \
    -c "select count(*) from information_schema.tables where table_schema = 'public'"
}

# The schema of the throw-away database without data; pg_dump writes a random \restrict token that is removed.
schema() {
  "${DOCKER:-docker}" compose exec -T database pg_dump --schema-only -U "${db_user}" -d "${bats_db}" | grep -v -E '^\\(un)?restrict '
}

@test "migrate twice: the second run applies nothing and changes nothing" {
  throwaway_database
  run make --no-print-directory migrate
  assert_success
  local before after
  before="$(dev_sql 'select version, checksum from _sqlx_migrations order by version')"
  run make --no-print-directory migrate
  assert_success
  assert_output --partial 'applied 0 migration(s)'
  after="$(dev_sql 'select version, checksum from _sqlx_migrations order by version')"
  [[ -n "${before}" ]]
  [[ "${before}" == "${after}" ]]
}

@test "migrate-rollback then migrate restores the schema" {
  throwaway_database
  run make --no-print-directory migrate
  assert_success
  local before rolled_back after
  before="$(schema)"
  run make --no-print-directory migrate-rollback
  assert_success
  rolled_back="$(schema)"
  [[ "${before}" != "${rolled_back}" ]]
  run make --no-print-directory migrate
  assert_success
  assert_output --partial 'applied 1 migration(s)'
  after="$(schema)"
  [[ -n "${before}" ]]
  [[ "${before}" == "${after}" ]]
}

@test "migrate and migrate-rollback leave the development database alone" {
  throwaway_database
  local dev_db dev_before dev_after
  dev_db="$(env_value DB_NAME)"
  dev_before="$(table_count "${dev_db}")"
  run make --no-print-directory migrate
  assert_success
  run make --no-print-directory migrate-rollback
  assert_success
  dev_after="$(table_count "${dev_db}")"
  [[ "${dev_before}" == "${dev_after}" ]]
}

@test "create-migration without name fails with the usage line and creates nothing" {
  local before after
  before="$(tree_status)"
  run make --no-print-directory create-migration
  assert_failure
  assert_output --partial 'Usage: make create-migration name=<snake_case_description>'
  assert_output --partial 'create-migration] Error 2'
  after="$(tree_status)"
  [[ "${before}" == "${after}" ]]
}
