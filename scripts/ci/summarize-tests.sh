#!/usr/bin/env bash
# Name each gate of the `tests` job and its result in the job summary.
#
# Usage: summarize-tests.sh
# Reads target/nextest/ci/junit.xml (the JUnit file of the coverage run) and target/coverage/lcov.info (the
# coverage data the 100% floor is applied to), and appends one line each for G11 (unit tests), G12 (integration
# tests) and G14 (coverage) to the file named by GITHUB_STEP_SUMMARY (standard output when it is not set).
# A test counts as an integration test when the nextest binary id of its suite ends in `::integration`; every
# other suite belongs to G11. G14 passes when the lcov data reports every line and every function as hit.
# Optional: TESTS_OUTCOME, the outcome of the step that ran `make tests-with-coverage doc-tests`; a failed step
# whose reports show nothing wrong is reported as a failure of G11 (the documentation tests).
# Exit status: 0 always; the result of a gate is in the text, not in the status.
set -euo pipefail

junit="${JUNIT_FILE:-target/nextest/ci/junit.xml}"
lcov="${LCOV_FILE:-target/coverage/lcov.info}"
summary="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

unit=passed
integration=passed
coverage=passed
unit_detail=
integration_detail=
coverage_detail=

if [ -f "$junit" ]; then
  # One tag per line, then: the suite of the current test case; a failure or error element marks that suite.
  failed=$(sed 's/</\n</g' "$junit" | awk '
    /^<testcase / {
      suite = $0
      sub(/.*classname="/, "", suite)
      sub(/".*/, "", suite)
      name = $0
      sub(/^<testcase name="/, "", name)
      sub(/".*/, "", name)
      current = suite "\t" name
      next
    }
    /^<(failure|error)[ >\/]/ { if (current != "") { print current; current = "" } }
  ')
  if [ -n "$failed" ]; then
    unit_count=$(printf '%s\n' "$failed" | awk -F'\t' '$1 !~ /::integration$/' | grep -c . || true)
    integration_count=$(printf '%s\n' "$failed" | awk -F'\t' '$1 ~ /::integration$/' | grep -c . || true)
    if [ "$unit_count" -gt 0 ]; then
      unit=failed
      unit_detail=" (${unit_count} failing)"
    fi
    if [ "$integration_count" -gt 0 ]; then
      integration=failed
      integration_detail=" (${integration_count} failing)"
    fi
  fi
else
  unit='not run'
  integration='not run'
  unit_detail=" (no ${junit})"
  integration_detail=" (no ${junit})"
fi

if [ ! -f "$junit" ]; then
  coverage='not run'
  coverage_detail=" (no test report)"
elif [ "$unit" = failed ] || [ "$integration" = failed ]; then
  # The coverage recipe stops at the first failing test run, so the floor was never applied.
  coverage='not run'
  coverage_detail=" (the test run failed before the floor was applied)"
elif [ ! -f "$lcov" ]; then
  coverage='not run'
  coverage_detail=" (no ${lcov})"
else
  totals=$(awk -F: '
    $1 == "LF" { lf += $2 } $1 == "LH" { lh += $2 }
    $1 == "FNF" { fnf += $2 } $1 == "FNH" { fnh += $2 }
    END { printf "%d %d %d %d", lf, lh, fnf, fnh }
  ' "$lcov")
  read -r lines_found lines_hit functions_found functions_hit <<<"$totals"
  if [ "$lines_found" -ne "$lines_hit" ] || [ "$functions_found" -ne "$functions_hit" ]; then
    coverage=failed
    coverage_detail=" (lines ${lines_hit}/${lines_found}, functions ${functions_hit}/${functions_found}; the floor is 100%)"
  else
    coverage_detail=" (lines ${lines_hit}/${lines_found}, functions ${functions_hit}/${functions_found})"
  fi
fi

if [ "${TESTS_OUTCOME-}" = failure ] && [ "$unit" = passed ] && [ "$integration" = passed ] && [ "$coverage" = passed ]; then
  unit=failed
  unit_detail=" (the documentation tests failed; see the job log)"
fi

{
  echo "- G11 Unit tests: ${unit}${unit_detail}"
  echo "- G12 Integration tests: ${integration}${integration_detail}"
  echo "- G14 Coverage floor: ${coverage}${coverage_detail}"
} >>"$summary"
