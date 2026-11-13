#!/usr/bin/env bash
# Remove everything aws-execute-load-tests.sh created (make aws-load-tests-cleanup): the resources named in
# tests/Load/aws-run.env, then the file itself. A resource that no longer exists is skipped, so the script can
# be run again after a partial cleanup, and without a run file there is nothing to do. The file stays when a
# removal fails, so that the next run retries.
# Options and environment: see config.sh. Exit status: 0 success; 1 an AWS call failed; 2 unusable configuration.
set -euo pipefail

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null # config.sh is the file sourced on the next line
. "${LOAD_DIR}/config.sh" "$@"
cd "${REPO_ROOT}"

export AWS_PAGER=""

# A security group can be deleted only after its instance's network interface is gone, which takes a short
# while after termination: 30 attempts, 10 seconds apart, are 5 minutes.
GROUP_DELETE_ATTEMPTS=30
GROUP_DELETE_INTERVAL=10

if [[ ! -f ${RUN_FILE} ]]; then
  echo "Nothing to clean up: ${RUN_FILE#"${REPO_ROOT}/"} not found."
  exit 0
fi

RUN_ID="" RECORDED_REGION="" RECORDED_MODE="" BUCKET_NAME="" ROLE_NAME="" INSTANCE_PROFILE_NAME=""
POLICY_NAME="" RECORDED_SECURITY_GROUP="" RECORDED_TAG="" INSTANCE_ID=""
while IFS='=' read -r key value || [[ -n ${key} ]]; do
  config_check "Value of ${key} in the run file" "${value}" '^[A-Za-z0-9._-]*$'
  case ${key} in
    RUN_ID) RUN_ID=${value} ;;
    REGION) RECORDED_REGION=${value} ;;
    LOCAL_MODE) RECORDED_MODE=${value} ;;
    BUCKET_NAME) BUCKET_NAME=${value} ;;
    ROLE_NAME) ROLE_NAME=${value} ;;
    INSTANCE_PROFILE_NAME) INSTANCE_PROFILE_NAME=${value} ;;
    POLICY_NAME) POLICY_NAME=${value} ;;
    SECURITY_GROUP_NAME) RECORDED_SECURITY_GROUP=${value} ;;
    INSTANCE_TAG) RECORDED_TAG=${value} ;;
    INSTANCE_ID) INSTANCE_ID=${value} ;;
    '') ;;
    *) config_fail "unknown key '${key}' in the run file." ;;
  esac
done <"${RUN_FILE}"

if [[ -z ${RUN_ID} || -z ${RECORDED_REGION} ]]; then
  config_fail "the run file is incomplete." 1
fi
if [[ ${RECORDED_MODE} != "${LOCAL_MODE}" ]]; then
  config_fail "the run was made with LOCAL_MODE_ENV=${RECORDED_MODE}; clean it up with the same setting." 1
fi
# The recorded region wins over the option and the environment: the resources live there.
REGION=${RECORDED_REGION}
AWS_REGION=${REGION}
export REGION AWS_REGION

echo "Cleaning up run ${RUN_ID} in ${REGION} (local mode ${LOCAL_MODE})"

# Without a working identity every lookup would report "not found" and the run file would be lost.
AWS_CLI sts get-caller-identity --query Account --output text >/dev/null

if [[ -n ${BUCKET_NAME} ]]; then
  if AWS_CLI s3api head-bucket --bucket "${BUCKET_NAME}" >/dev/null 2>&1; then
    echo "Deleting S3 bucket and its contents: ${BUCKET_NAME}"
    AWS_CLI s3 rm "s3://${BUCKET_NAME}" --recursive >/dev/null
    AWS_CLI s3 rb "s3://${BUCKET_NAME}"
  else
    echo "S3 bucket ${BUCKET_NAME} not found, skipping."
  fi
fi

if [[ -n ${RECORDED_TAG} ]]; then
  INSTANCE_IDS="$(AWS_CLI ec2 describe-instances \
    --filters "Name=tag:Name,Values=${RECORDED_TAG}" "Name=instance-state-name,Values=pending,running,stopping,stopped" \
    --query "Reservations[*].Instances[*].InstanceId" --output text)"
  # The recorded id may no longer match the tag filter; it is added only when it still exists in a live state.
  if [[ -n ${INSTANCE_ID} && " ${INSTANCE_IDS//$'\t'/ } " != *" ${INSTANCE_ID} "* ]]; then
    INSTANCE_IDS="${INSTANCE_IDS:+${INSTANCE_IDS}$'\t'}$(AWS_CLI ec2 describe-instances --instance-ids "${INSTANCE_ID}" \
      --filters "Name=instance-state-name,Values=pending,running,stopping,stopped" \
      --query "Reservations[*].Instances[*].InstanceId" --output text 2>/dev/null || true)"
  fi
  read -r -a ids <<<"${INSTANCE_IDS//$'\t'/ }"
  if ((${#ids[@]} > 0)); then
    echo "Terminating EC2 instances: ${ids[*]}"
    AWS_CLI ec2 terminate-instances --instance-ids "${ids[@]}" >/dev/null
    # The built-in waiter polls every 15 seconds and gives up after 40 attempts (10 minutes).
    AWS_CLI ec2 wait instance-terminated --instance-ids "${ids[@]}"
    echo "Instances terminated."
  else
    echo "No live instances found with tag: ${RECORDED_TAG}"
  fi
fi

if [[ -n ${POLICY_NAME} ]]; then
  POLICY_ARN="$(AWS_CLI iam list-policies --scope Local --query "Policies[?PolicyName=='${POLICY_NAME}'].Arn" --output text)"
  if [[ -n ${POLICY_ARN} && ${POLICY_ARN} != None ]]; then
    if [[ -n ${ROLE_NAME} ]] && AWS_CLI iam get-role --role-name "${ROLE_NAME}" >/dev/null 2>&1; then
      AWS_CLI iam detach-role-policy --role-name "${ROLE_NAME}" --policy-arn "${POLICY_ARN}"
    fi
    AWS_CLI iam delete-policy --policy-arn "${POLICY_ARN}"
    echo "Deleted policy: ${POLICY_NAME}"
  else
    echo "Policy ${POLICY_NAME} not found, skipping."
  fi
fi

if [[ -n ${INSTANCE_PROFILE_NAME} ]]; then
  if AWS_CLI iam get-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}" >/dev/null 2>&1; then
    if [[ -n ${ROLE_NAME} ]] && AWS_CLI iam get-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}" \
      --query "InstanceProfile.Roles[?RoleName=='${ROLE_NAME}'].RoleName" --output text | grep -q .; then
      AWS_CLI iam remove-role-from-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}" --role-name "${ROLE_NAME}"
    fi
    AWS_CLI iam delete-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}"
    echo "IAM instance profile ${INSTANCE_PROFILE_NAME} deleted."
  else
    echo "IAM instance profile ${INSTANCE_PROFILE_NAME} not found, skipping."
  fi
fi

if [[ -n ${ROLE_NAME} ]]; then
  if AWS_CLI iam get-role --role-name "${ROLE_NAME}" >/dev/null 2>&1; then
    AWS_CLI iam delete-role --role-name "${ROLE_NAME}"
    echo "IAM role ${ROLE_NAME} deleted."
  else
    echo "IAM role ${ROLE_NAME} not found, skipping."
  fi
fi

if [[ -n ${RECORDED_SECURITY_GROUP} ]]; then
  GROUP_ID="$(AWS_CLI ec2 describe-security-groups --filters "Name=group-name,Values=${RECORDED_SECURITY_GROUP}" \
    --query "SecurityGroups[0].GroupId" --output text)"
  if [[ -n ${GROUP_ID} && ${GROUP_ID} != None ]]; then
    echo "Deleting security group: ${RECORDED_SECURITY_GROUP}"
    deleted=false
    for ((attempt = 1; attempt <= GROUP_DELETE_ATTEMPTS; attempt++)); do
      if AWS_CLI ec2 delete-security-group --group-id "${GROUP_ID}"; then
        deleted=true
        break
      fi
      if ((attempt < GROUP_DELETE_ATTEMPTS)); then
        echo "The security group is still in use. Retrying in ${GROUP_DELETE_INTERVAL} seconds... (Attempt ${attempt}/${GROUP_DELETE_ATTEMPTS})"
        sleep "${GROUP_DELETE_INTERVAL}"
      fi
    done
    if [[ ${deleted} != true ]]; then
      echo "Error: could not delete security group ${RECORDED_SECURITY_GROUP} after ${GROUP_DELETE_ATTEMPTS} attempts." >&2
      exit 1
    fi
    echo "Security group ${RECORDED_SECURITY_GROUP} deleted."
  else
    echo "Security group ${RECORDED_SECURITY_GROUP} not found, skipping."
  fi
fi

rm -f -- "${RUN_FILE}"
echo "Cleanup complete!"
