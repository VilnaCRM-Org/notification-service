#!/usr/bin/env bash
# Export the NAME=value pairs of the given env files, in order, then run a command.
#
# Usage: with-env.sh <env-file>... -- <command>...
#
# A later file overrides an earlier one. A variable that the caller has already set is never changed, by
# any file. `${NAME}` references in a value are expanded from the environment as it is at that point.
# A line is `NAME=value` (an `export ` prefix is accepted). Values are read like the service (dotenvy) and Docker
# Compose read them, for the forms that both read the same way: a value in double quotes loses the quotes (`\"`,
# `\\` and `\n` are unescaped, `${NAME}` is expanded); a value in single quotes loses the quotes and stays literal;
# an unquoted value ends at the first whitespace followed by `#` (an inline comment). A quoted value must close on
# its own line, and only whitespace or a `# comment` may follow the closing quote. The forms the two read
# differently are refused, so that one line never has two meanings: an unquoted value with whitespace, a quote or a
# backslash inside, a `$` that does not start `${NAME}`, an unknown escape in double quotes, and a comment right
# after the `=`.
# Exit status: the command's; 2 when a file is missing, the separator is absent, no command follows it or a line is
# not valid.
set -euo pipefail

usage() {
  echo "usage: with-env.sh <env-file>... -- <command>..." >&2
  exit 2
}

# Expand every ${NAME} of $1 once, left to right; an unset variable expands to nothing.
expand() {
  local rest=$1 out='' ref token
  while [[ $rest =~ \$\{([A-Za-z_][A-Za-z0-9_]*)\} ]]; do
    token=${BASH_REMATCH[0]}
    ref=${BASH_REMATCH[1]}
    out+=${rest%%"$token"*}${!ref-}
    rest=${rest#*"$token"}
  done
  printf '%s' "$out$rest"
}

# Succeeds when every dollar sign of $1 starts a `${NAME}` reference; other forms (`$NAME`, `${NAME:-x}`, `$$`) are
# read differently by dotenvy and Compose, so they are refused.
only_simple_references() {
  local rest=$1
  while [[ $rest =~ \$\{[A-Za-z_][A-Za-z0-9_]*\} ]]; do
    rest=${rest/"${BASH_REMATCH[0]}"/}
  done
  [[ $rest != *'$'* ]]
}

# Unescapes the inside of a double-quoted value in one pass: \" \\ and \n; any other escape is an error (dotenvy
# refuses it too). Prints the result and returns 1 on an unknown escape.
unescape() {
  local rest=$1 out='' char next backslash=$'\\'
  while [[ -n $rest ]]; do
    char=${rest:0:1}
    rest=${rest:1}
    if [[ $char == "$backslash" ]]; then
      next=${rest:0:1}
      case $next in
        n) out+=$'\n' ;;
        '"' | "$backslash") out+=$next ;;
        *) return 1 ;;
      esac
      rest=${rest:1}
    else
      out+=$char
    fi
  done
  printf '%s' "$out"
}

# Reads the text after the `=` of a line into the variable `value`. Returns 1 for a form that the service (dotenvy)
# and Docker Compose read differently, so that one line never means two values: a quoted value that does not close
# or is followed by something other than a comment, an unquoted value that holds whitespace, a quote, a backslash or a
# `$` that does not start `${NAME}`, an unknown escape in double quotes, and a comment right after the `=`.
parse_value() {
  local raw=$1 body
  [[ ! $raw =~ ^[[:space:]]+# ]] || return 1
  raw=${raw#"${raw%%[![:space:]]*}"}
  case $raw in
    '"'*)
      body=${raw#\"}
      [[ $body =~ ^((\\.|[^\"\\])*)\"([[:space:]]+#.*|[[:space:]]*)$ ]] || return 1
      body=${BASH_REMATCH[1]}
      only_simple_references "$body" || return 1
      body=$(unescape "$body") || return 1
      value=$(expand "$body")
      ;;
    "'"*)
      body=${raw#\'}
      [[ $body =~ ^([^\']*)\'([[:space:]]+#.*|[[:space:]]*)$ ]] || return 1
      value=${BASH_REMATCH[1]}
      ;;
    *)
      body=${raw%%[[:space:]]#*}
      body=${body%"${body##*[![:space:]]}"}
      [[ $body != *[[:space:]\"\'\\]* ]] || return 1
      only_simple_references "$body" || return 1
      value=$(expand "$body")
      ;;
  esac
}

files=()
while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
  files+=("$1")
  shift
done
[ "$#" -gt 0 ] || { echo "with-env.sh: the separator '--' is missing" >&2; usage; }
shift
[ "${#files[@]}" -gt 0 ] || { echo "with-env.sh: no env file given" >&2; usage; }
[ "$#" -gt 0 ] || { echo "with-env.sh: no command given" >&2; usage; }

for file in "${files[@]}"; do
  [ -f "$file" ] || { echo "with-env.sh: env file not found: $file" >&2; exit 2; }
done

# Names the caller has set: no file may change them.
declare -A caller=()
while IFS= read -r name; do
  caller[$name]=1
done < <(compgen -e)

for file in "${files[@]}"; do
  while IFS= read -r line || [ -n "$line" ]; do
    line=${line%$'\r'}
    line=${line#"${line%%[![:space:]]*}"}
    case $line in '' | '#'*) continue ;; esac
    line=${line#export[[:space:]]}
    name=${line%%=*}
    name=${name%"${name##*[![:space:]]}"}
    [[ $name =~ ^[A-Za-z_][A-Za-z0-9_]*$ && $line == *=* ]] || { echo "with-env.sh: invalid line in $file: $line" >&2; exit 2; }
    [ -z "${caller[$name]+set}" ] || continue
    parse_value "${line#*=}" || { echo "with-env.sh: invalid value in $file: $line" >&2; exit 2; }
    export "$name=$value"
  done <"$file"
done

exec "$@"
