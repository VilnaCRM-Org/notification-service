#!/usr/bin/env bash
# Shared configuration of aws-execute-load-tests.sh and cleanup.sh. Source it with the script's arguments:
#   . "${LOAD_DIR}/config.sh" "$@"
#
# Options: -r region, -a ami, -t instance type, -i instance tag, -o role, -b branch, -u repository URL, -s security
#          group, -l load target (the Make target that runs on the instance).
# Environment: LOCAL_MODE_ENV=true talks to LocalStack with the placeholder credentials `test`; otherwise
#   AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY (and AWS_SESSION_TOKEN for temporary credentials) are
#   required. AWS_REGION is used when -r is not given; without both the region is us-east-1.
#   AWS_CLI_IMAGE (read from make/tool-versions.mk when unset) is the AWS CLI image.
#
# Provides: the variables below (exported), the function AWS_CLI (every AWS call goes through it: the CLI runs
# from the pinned image, so neither aws, uuidgen nor envsubst is needed on the host), `render` (sed rendering
# of a template into WORK_DIR) and `resolve_repository`.
# Exit status of a sourcing script: 2 when the configuration is unusable.

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${LOAD_DIR}/../.." && pwd)"
RUN_FILE="${LOAD_DIR}/aws-run.env"

DEFAULT_REGION="us-east-1"
# The fixed placeholder AMI of the LocalStack dry run (the emulator does not look at it).
LOCAL_AMI_ID="ami-0e86e20dae9224db8"
# Public SSM parameter of Canonical: the current Ubuntu 24.04 AMI of the chosen region.
AMI_PARAMETER="/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id"
# 4 vCPU and 8 GiB: a cold release build of the image takes about 5 minutes, which t2.micro cannot do.
DEFAULT_INSTANCE_TYPE="c6i.xlarge"
DEFAULT_INSTANCE_TAG="LoadTestInstance"
DEFAULT_ROLE_NAME="EC2S3WriteAccessRole"
DEFAULT_SECURITY_GROUP_NAME="LoadTestSecurityGroup"
DEFAULT_LOAD_TARGET="smoke-load-tests"

config_usage="Usage: ${0##*/} [-r region] [-a ami_id] [-t instance_type] [-i instance_tag] [-o role_name] [-b branch_name] [-u repository_url] [-s security_group_name] [-l load_target]"

config_fail() {
  echo "Error: $1" >&2
  exit "${2:-2}"
}

# config_check <what> <value> <extended regular expression>
config_check() {
  [[ $2 =~ $3 ]] || config_fail "$1 '$2' is not valid."
}

for tool in docker git; do
  command -v "${tool}" >/dev/null 2>&1 || config_fail "${tool} is required and was not found on PATH."
done

# The options are the only input of these variables: clear them, so that a variable of the same name in
# the caller's environment cannot take the place of an option that was not given.
REGION="" AMI_ID="" INSTANCE_TYPE="" INSTANCE_TAG="" ROLE_NAME="" BRANCH_NAME="" REPO_URL="" SECURITY_GROUP_NAME="" LOAD_TARGET=""

OPTIND=1
while getopts "r:a:t:i:o:b:u:s:l:" opt; do
  case ${opt} in
    r) REGION=${OPTARG} ;;
    a) AMI_ID=${OPTARG} ;;
    t) INSTANCE_TYPE=${OPTARG} ;;
    i) INSTANCE_TAG=${OPTARG} ;;
    o) ROLE_NAME=${OPTARG} ;;
    b) BRANCH_NAME=${OPTARG} ;;
    u) REPO_URL=${OPTARG} ;;
    s) SECURITY_GROUP_NAME=${OPTARG} ;;
    l) LOAD_TARGET=${OPTARG} ;;
    *)
      echo "${config_usage}" >&2
      exit 2
      ;;
  esac
done

# Precedence of the region: the option, then AWS_REGION of the caller's environment, then us-east-1.
REGION=${REGION:-${AWS_REGION:-${DEFAULT_REGION}}}
AMI_ID=${AMI_ID:-}
INSTANCE_TYPE=${INSTANCE_TYPE:-${DEFAULT_INSTANCE_TYPE}}
INSTANCE_TAG=${INSTANCE_TAG:-${DEFAULT_INSTANCE_TAG}}
ROLE_NAME=${ROLE_NAME:-${DEFAULT_ROLE_NAME}}
SECURITY_GROUP_NAME=${SECURITY_GROUP_NAME:-${DEFAULT_SECURITY_GROUP_NAME}}
LOAD_TARGET=${LOAD_TARGET:-${DEFAULT_LOAD_TARGET}}
BRANCH_NAME=${BRANCH_NAME:-}
REPO_URL=${REPO_URL:-}
LOCAL_MODE=${LOCAL_MODE_ENV:-false}

# Values reach AWS calls and the instance's shell script: allow only plain characters.
config_check "Region" "${REGION}" '^[a-z]{2}(-[a-z]+)+-[0-9]+$'
config_check "AMI id" "${AMI_ID}" '^(ami-[0-9a-f]+)?$'
config_check "Instance type" "${INSTANCE_TYPE}" '^[a-z0-9-]+\.[a-z0-9]+$'
config_check "Instance tag" "${INSTANCE_TAG}" '^[A-Za-z0-9][A-Za-z0-9._-]{0,40}$'
config_check "Role name" "${ROLE_NAME}" '^[A-Za-z0-9][A-Za-z0-9._-]{0,40}$'
config_check "Security group name" "${SECURITY_GROUP_NAME}" '^[A-Za-z0-9][A-Za-z0-9._-]{0,40}$'
config_check "Load target" "${LOAD_TARGET}" '^[A-Za-z0-9][A-Za-z0-9_-]*$'
config_check "Branch name" "${BRANCH_NAME}" '^([A-Za-z0-9][A-Za-z0-9._/-]*)?$'

# The AWS CLI image is pinned in make/tool-versions.mk (lines of the form `export NAME := value`).
if [[ -z ${AWS_CLI_IMAGE:-} ]]; then
  AWS_CLI_IMAGE="$(sed -n 's/^export AWS_CLI_IMAGE[[:space:]]*:=[[:space:]]*//p' "${REPO_ROOT}/make/tool-versions.mk" | head -n 1)"
fi
[[ -n ${AWS_CLI_IMAGE} ]] || config_fail "AWS_CLI_IMAGE is not set and not found in make/tool-versions.mk."

case ${LOCAL_MODE} in
  true)
    # LOCALSTACK_PORT comes from the environment or, with the semantics of scripts/with-env.sh, from .env.
    if [[ -z ${LOCALSTACK_PORT:-} && -f "${REPO_ROOT}/.env" ]]; then
      LOCALSTACK_PORT="$("${REPO_ROOT}/scripts/with-env.sh" "${REPO_ROOT}/.env" -- printenv LOCALSTACK_PORT || true)"
    fi
    LOCALSTACK_PORT=${LOCALSTACK_PORT:-4566}
    config_check "LOCALSTACK_PORT" "${LOCALSTACK_PORT}" '^[0-9]+$'
    ENDPOINT_URL="http://localhost:${LOCALSTACK_PORT}"
    AWS_ACCESS_KEY_ID='test'
    AWS_SECRET_ACCESS_KEY='test'
    unset AWS_SESSION_TOKEN
    ;;
  false)
    ENDPOINT_URL=""
    if [[ -z ${AWS_ACCESS_KEY_ID:-} || -z ${AWS_SECRET_ACCESS_KEY:-} ]]; then
      config_fail "AWS credentials are missing. Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY (and AWS_SESSION_TOKEN for temporary credentials) in the environment, or use LOCAL_MODE_ENV=true for the LocalStack dry run."
    fi
    ;;
  *) config_fail "LOCAL_MODE_ENV must be true or false, not '${LOCAL_MODE}'." ;;
esac
AWS_REGION=${REGION}
export REGION AMI_ID INSTANCE_TYPE INSTANCE_TAG ROLE_NAME SECURITY_GROUP_NAME LOAD_TARGET BRANCH_NAME LOCAL_MODE
export ENDPOINT_URL AWS_CLI_IMAGE AWS_REGION AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY LOAD_DIR REPO_ROOT RUN_FILE
export LOCAL_AMI_ID AMI_PARAMETER

# The temporary directory that holds the rendered files; it is mounted at /work in the AWS CLI container.
WORK_DIR="$(mktemp -d)"
export WORK_DIR
trap 'rm -rf -- "${WORK_DIR:?}"' EXIT

# AWS_CLI <aws arguments>: runs the AWS CLI from the pinned image. Files are read as file://<name> from /work.
AWS_CLI() {
  docker run --rm --network host --user "$(id -u):$(id -g)" -e HOME=/tmp \
    -e AWS_ACCESS_KEY_ID -e AWS_SECRET_ACCESS_KEY -e AWS_SESSION_TOKEN -e AWS_REGION \
    -v "${WORK_DIR}:/work" -w /work "${AWS_CLI_IMAGE}" \
    ${ENDPOINT_URL:+--endpoint-url "${ENDPOINT_URL}"} "$@"
}

# sed_escape <value>: escape a value for the replacement of an s command that uses | as its delimiter.
sed_escape() {
  printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'
}

# render <template> <output name> <NAME=value>...: replace the placeholders $NAME and @@NAME@@ with sed and
# write the file to WORK_DIR/<output name>.
render() {
  local template=$1 output=$2 pair name value
  shift 2
  local -a expressions=()
  for pair in "$@"; do
    name=${pair%%=*}
    value=$(sed_escape "${pair#*=}")
    expressions+=(-e "s|\\\$${name}|${value}|g" -e "s|@@${name}@@|${value}|g")
  done
  sed "${expressions[@]}" "${template}" >"${WORK_DIR}/${output}"
}

# https_url <http or https URL>: the HTTPS form without credentials (one rule for -u and for the remote origin).
https_url() {
  local rest host path
  rest=${1#*://}
  host=${rest%%/*}
  path=${rest#"${host}"}
  printf 'https://%s%s' "${host##*@}" "${path}"
}

# resolve_repository: REPO_URL (HTTPS, without credentials) from -u when it was given (the remote is then not
# read), otherwise from `git remote get-url origin`; the branch from `git rev-parse --abbrev-ref HEAD` unless
# -b was given.
resolve_repository() {
  local url rest host path
  if [[ -n ${REPO_URL} ]]; then
    case ${REPO_URL} in
      https://* | http://*) REPO_URL=$(https_url "${REPO_URL}") ;;
      *) config_fail "the repository URL given with -u must be an http or https URL." ;;
    esac
  else
    url=$(git -C "${REPO_ROOT}" remote get-url origin 2>/dev/null) || config_fail "the git remote 'origin' is not set; give the repository URL with -u (make aws-load-tests repo_url=<https url>)."
    case ${url} in
      https://* | http://*) REPO_URL=$(https_url "${url}") ;;
      ssh://*)
        rest=${url#ssh://}
        host=${rest%%/*}
        path=${rest#"${host}"}
        host=${host##*@}
        REPO_URL="https://${host%%:*}${path}"
        ;;
      *@*:*)
        rest=${url#*@}
        REPO_URL="https://${rest%%:*}/${rest#*:}"
        ;;
      *) config_fail "the URL of the remote 'origin' cannot be converted to HTTPS; give the repository URL with -u (make aws-load-tests repo_url=<https url>)." ;;
    esac
  fi
  config_check "Repository URL" "${REPO_URL}" '^https://[A-Za-z0-9.-]+(:[0-9]+)?/[A-Za-z0-9._~/-]+$'
  if [[ -z ${BRANCH_NAME} ]]; then
    BRANCH_NAME=$(git -C "${REPO_ROOT}" rev-parse --abbrev-ref HEAD)
    [[ ${BRANCH_NAME} != HEAD ]] || config_fail "HEAD is detached; give the branch with -b (make aws-load-tests branch=<name>)."
    config_check "Branch name" "${BRANCH_NAME}" '^[A-Za-z0-9][A-Za-z0-9._/-]*$'
  fi
  export REPO_URL BRANCH_NAME
}
