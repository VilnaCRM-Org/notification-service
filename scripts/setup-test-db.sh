#!/usr/bin/env bash
# Drop, create and migrate the test database.
#
# Usage: setup-test-db.sh
# Reads DATABASE_URL (the test database, from .env.test through scripts/with-env.sh). Refuses, before it touches
# anything, a database whose name does not end in `_test`: the reset drops the database. Then checks that `sqlx` exists and
# that the database server accepts connections (10 attempts, 1 s apart), then runs exactly
# `sqlx database reset -y -f --source migrations`.
# Exit status: 0 success; 1 the reset failed; 2 the database is not a test database, is unreachable or `sqlx` is missing.
set -euo pipefail

attempts=10

[ -n "${DATABASE_URL-}" ] || { echo "setup-test-db.sh: DATABASE_URL is not set" >&2; exit 2; }

# The name of the database is the path of the URL without its query. Only a name ending in `_test` may be dropped.
database_path=${DATABASE_URL#*://}
database_path=${database_path%%\?*}
case $database_path in */*) database=${database_path#*/} ;; *) database='' ;; esac
case $database in
  ?*_test) ;;
  *)
    echo "setup-test-db.sh: refusing to reset the database '${database}': the name of a test database must end in _test" >&2
    exit 2
    ;;
esac

command -v sqlx >/dev/null 2>&1 || { echo "setup-test-db.sh: sqlx is not installed (cargo install sqlx-cli)" >&2; exit 2; }

# `pg_isready` is not assumed: a throw-away `sqlx` call would need the database to exist, so the server is probed
# with a TCP connection to the host and port of the URL.
authority=${DATABASE_URL#*://}
authority=${authority#*@}
authority=${authority%%/*}
host=${authority%%:*}
port=${authority##*:}
[ "$port" != "$authority" ] || port=5432

reachable=false
for attempt in $(seq 1 "$attempts"); do
  if (exec 3<>"/dev/tcp/$host/$port") 2>/dev/null; then
    reachable=true
    break
  fi
  echo "setup-test-db.sh: database not reachable (attempt $attempt of $attempts)" >&2
  [ "$attempt" -eq "$attempts" ] || sleep 1
done
$reachable || { echo "setup-test-db.sh: the database did not accept connections after $attempts attempts" >&2; exit 2; }

sqlx database reset -y -f --source migrations || exit 1
