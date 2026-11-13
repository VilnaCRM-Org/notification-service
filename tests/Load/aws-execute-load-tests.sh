#!/usr/bin/env bash
# Run the load tests on a temporary EC2 instance (make aws-load-tests), or against LocalStack with
# LOCAL_MODE_ENV=true. The host needs Docker and git only. Options and environment: see config.sh.
#
# The names of every resource carry a run id and are written to tests/Load/aws-run.env before the first
# resource is created; cleanup.sh (make aws-load-tests-cleanup) reads that file.
# Set wait=false to launch the instance without waiting for the results.
# Exit status: 0 success (also the LocalStack dry run); 1 a previous run was not cleaned up, there is
# no default VPC, the bucket policy or the instance launch failed after its retries, or the run
# failed or timed out; any other failed AWS CLI call passes its own status through (255); 2 the
# configuration is unusable.
set -euo pipefail

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null # config.sh is the file sourced on the next line
. "${LOAD_DIR}/config.sh" "$@"
cd "${REPO_ROOT}"
resolve_repository

export AWS_PAGER=""

# IAM is eventually consistent: a new role can take up to about 4.4 minutes to be accepted as a principal of
# a bucket policy. 53 attempts, 5 seconds apart, are 265 seconds.
BUCKET_POLICY_ATTEMPTS=53
BUCKET_POLICY_INTERVAL=5
# A new instance profile is accepted by run-instances within about a minute: 24 attempts, 5 seconds apart,
# are 2 minutes.
LAUNCH_ATTEMPTS=24
LAUNCH_INTERVAL=5
# The instance builds the image and runs the load profiles in well under 45 minutes. The wait polls every
# 30 seconds: 90 polls are 45 minutes.
RESULT_POLL_LIMIT=90
RESULT_POLL_INTERVAL=30

INSTANCE_ID=""
finish() {
  local status=$?
  if [[ ${status} -ne 0 && -f ${RUN_FILE} ]]; then
    hint="make aws-load-tests-cleanup"
    [[ ${LOCAL_MODE} != true ]] || hint="${hint} LOCAL_MODE_ENV=true"
    echo "The run did not complete. Remove what it created with: ${hint}" >&2
  fi
  rm -rf -- "${WORK_DIR:?}"
}
trap finish EXIT

if [[ -f ${RUN_FILE} ]]; then
  echo "Error: ${RUN_FILE} exists: a previous run was not cleaned up. Run make aws-load-tests-cleanup first." >&2
  exit 1
fi

# 8 hexadecimal characters from the kernel's random source.
RUN_ID="$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')"
BUCKET_NAME="loadtest-bucket-${RUN_ID}"
ROLE_NAME="${ROLE_NAME}-${RUN_ID}"
INSTANCE_PROFILE_NAME="${ROLE_NAME}"
POLICY_NAME="S3WriteAccessToBucket-${RUN_ID}"
SECURITY_GROUP_NAME="${SECURITY_GROUP_NAME}-${RUN_ID}"
INSTANCE_TAG="${INSTANCE_TAG}-${RUN_ID}"

cat >"${RUN_FILE}" <<EOF
RUN_ID=${RUN_ID}
REGION=${REGION}
LOCAL_MODE=${LOCAL_MODE}
BUCKET_NAME=${BUCKET_NAME}
ROLE_NAME=${ROLE_NAME}
INSTANCE_PROFILE_NAME=${INSTANCE_PROFILE_NAME}
POLICY_NAME=${POLICY_NAME}
SECURITY_GROUP_NAME=${SECURITY_GROUP_NAME}
INSTANCE_TAG=${INSTANCE_TAG}
EOF

echo "Run id: ${RUN_ID} (region ${REGION}, local mode ${LOCAL_MODE})"
echo "Repository URL: ${REPO_URL}"
echo "Repository branch: ${BRANCH_NAME}"
echo "Resource names are recorded in ${RUN_FILE#"${REPO_ROOT}/"}"

if [[ -z ${AMI_ID} ]]; then
  if [[ ${LOCAL_MODE} == true ]]; then
    AMI_ID=${LOCAL_AMI_ID}
  else
    AMI_ID="$(AWS_CLI ssm get-parameter --name "${AMI_PARAMETER}" --query Parameter.Value --output text)"
  fi
fi
echo "Instance type: ${INSTANCE_TYPE}, AMI: ${AMI_ID}"

VPC_ID="$(AWS_CLI ec2 describe-vpcs --filters "Name=isDefault,Values=true" --query "Vpcs[0].VpcId" --output text)"
if [[ ${VPC_ID} == None || -z ${VPC_ID} ]]; then
  echo "Error: Default VPC not found in region ${REGION}." >&2
  exit 1
fi
echo "Using VPC ID: ${VPC_ID}"

echo "Creating security group: ${SECURITY_GROUP_NAME}"
SECURITY_GROUP="$(AWS_CLI ec2 create-security-group --group-name "${SECURITY_GROUP_NAME}" \
  --description "Security group for load testing" --vpc-id "${VPC_ID}" --query GroupId --output text)"

echo "Creating S3 bucket: ${BUCKET_NAME}"
AWS_CLI s3 mb "s3://${BUCKET_NAME}"

echo "Creating IAM role: ${ROLE_NAME}"
cp "${LOAD_DIR}/trust-policy.json" "${WORK_DIR}/trust-policy.json"
AWS_CLI iam create-role --role-name "${ROLE_NAME}" --assume-role-policy-document file://trust-policy.json \
  --query Role.Arn --output text

ACCOUNT_ID="$(AWS_CLI sts get-caller-identity --query Account --output text)"

# The templates hold the placeholders $ACCOUNT_ID, $ROLE_NAME and $BUCKET_NAME.
policy_values=("ACCOUNT_ID=${ACCOUNT_ID}" "ROLE_NAME=${ROLE_NAME}" "BUCKET_NAME=${BUCKET_NAME}")
render "${LOAD_DIR}/s3-bucket-policy.json" s3-bucket-policy-filled.json "${policy_values[@]}"
render "${LOAD_DIR}/s3-access-policy.json" s3-access-policy-filled.json "${policy_values[@]}"

echo "Applying bucket policy to ${BUCKET_NAME}..."
applied=false
for ((attempt = 1; attempt <= BUCKET_POLICY_ATTEMPTS; attempt++)); do
  if AWS_CLI s3api put-bucket-policy --bucket "${BUCKET_NAME}" --policy file://s3-bucket-policy-filled.json; then
    applied=true
    break
  fi
  if ((attempt < BUCKET_POLICY_ATTEMPTS)); then
    echo "Failed to apply bucket policy. Retrying in ${BUCKET_POLICY_INTERVAL} seconds... (Attempt ${attempt}/${BUCKET_POLICY_ATTEMPTS})"
    sleep "${BUCKET_POLICY_INTERVAL}"
  fi
done
if [[ ${applied} != true ]]; then
  echo "Error: Failed to apply bucket policy after ${BUCKET_POLICY_ATTEMPTS} attempts." >&2
  exit 1
fi
echo "Bucket policy applied successfully."

echo "Creating IAM policy: ${POLICY_NAME}"
POLICY_ARN="$(AWS_CLI iam create-policy --policy-name "${POLICY_NAME}" --policy-document file://s3-access-policy-filled.json \
  --query Policy.Arn --output text)"
AWS_CLI iam attach-role-policy --role-name "${ROLE_NAME}" --policy-arn "${POLICY_ARN}"

echo "Creating IAM instance profile: ${INSTANCE_PROFILE_NAME}"
AWS_CLI iam create-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}" --query InstanceProfile.Arn --output text
AWS_CLI iam add-role-to-instance-profile --instance-profile-name "${INSTANCE_PROFILE_NAME}" --role-name "${ROLE_NAME}"

render "${LOAD_DIR}/user-data.sh" user-data.sh "REPO_URL=${REPO_URL}" "BRANCH_NAME=${BRANCH_NAME}" \
  "LOAD_TARGET=${LOAD_TARGET}" "BUCKET_NAME=${BUCKET_NAME}" "RUN_ID=${RUN_ID}" "REGION=${REGION}"

echo "Launching the instance..."
for ((attempt = 1; attempt <= LAUNCH_ATTEMPTS; attempt++)); do
  if INSTANCE_ID="$(AWS_CLI ec2 run-instances --image-id "${AMI_ID}" --instance-type "${INSTANCE_TYPE}" \
    --security-group-ids "${SECURITY_GROUP}" --iam-instance-profile "Name=${INSTANCE_PROFILE_NAME}" \
    --user-data file://user-data.sh \
    --block-device-mappings '[{"DeviceName":"/dev/sda1","Ebs":{"VolumeSize":30}}]' \
    --instance-initiated-shutdown-behavior terminate \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=${INSTANCE_TAG}}]" \
    --query "Instances[0].InstanceId" --output text)"; then
    break
  fi
  INSTANCE_ID=""
  if ((attempt < LAUNCH_ATTEMPTS)); then
    echo "Failed to launch the instance. Retrying in ${LAUNCH_INTERVAL} seconds... (Attempt ${attempt}/${LAUNCH_ATTEMPTS})"
    sleep "${LAUNCH_INTERVAL}"
  fi
done
if [[ -z ${INSTANCE_ID} || ${INSTANCE_ID} == None ]]; then
  echo "Error: Failed to launch EC2 instance." >&2
  exit 1
fi
echo "INSTANCE_ID=${INSTANCE_ID}" >>"${RUN_FILE}"

echo "Launched instance: ${INSTANCE_ID}"
echo "You can access the S3 bucket here: https://${REGION}.console.aws.amazon.com/s3/buckets/${BUCKET_NAME}?region=${REGION}&bucketType=general"

if [[ ${LOCAL_MODE} == true ]]; then
  echo "Local mode: the emulator does not execute user data, so the wait for results is skipped."
  exit 0
fi
if [[ ${wait:-true} == false ]]; then
  echo "wait=false: not waiting for the results."
  exit 0
fi

echo "Waiting for the instance to finish (at most $((RESULT_POLL_LIMIT * RESULT_POLL_INTERVAL / 60)) minutes)..."
outcome=""
for ((poll = 1; poll <= RESULT_POLL_LIMIT; poll++)); do
  keys="$(AWS_CLI s3api list-objects-v2 --bucket "${BUCKET_NAME}" --prefix "${RUN_ID}/" --query 'Contents[].Key' --output text 2>/dev/null || true)"
  keys=" ${keys//$'\t'/ } "
  if [[ ${keys} == *" ${RUN_ID}/FAILED "* ]]; then
    outcome=FAILED
  elif [[ ${keys} == *" ${RUN_ID}/DONE "* ]]; then
    outcome=DONE
  fi
  [[ -z ${outcome} ]] || break
  if ((poll < RESULT_POLL_LIMIT)); then
    sleep "${RESULT_POLL_INTERVAL}"
  fi
done

if [[ -z ${outcome} ]]; then
  echo "Error: no result after $((RESULT_POLL_LIMIT * RESULT_POLL_INTERVAL / 60)) minutes. Terminating instance ${INSTANCE_ID}." >&2
  AWS_CLI ec2 terminate-instances --instance-ids "${INSTANCE_ID}" >/dev/null
  exit 1
fi

RESULTS_DIR="${LOAD_DIR}/results/aws/${RUN_ID}"
mkdir -p "${RESULTS_DIR}"
AWS_CLI s3 cp "s3://${BUCKET_NAME}/${RUN_ID}/" /work/results --recursive >/dev/null
cp -R "${WORK_DIR}/results/." "${RESULTS_DIR}/"
echo "Results downloaded to ${RESULTS_DIR#"${REPO_ROOT}/"}"

if [[ ${outcome} == FAILED ]]; then
  echo "Error: the load tests failed on the instance; see the cloud-init log in the results." >&2
  exit 1
fi
echo "The load tests finished."
