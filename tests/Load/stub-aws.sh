#!/usr/bin/env bash
# A stub of the AWS CLI for the dry run of the EC2 load-test runner (make aws-load-tests LOCAL_MODE_ENV=true and
# make aws-load-tests-cleanup LOCAL_MODE_ENV=true). config.sh calls it in place of the AWS CLI image when
# LOCAL_MODE_ENV is true, so the dry run needs neither AWS, credentials, an emulator nor Docker.
#
# It answers exactly the calls of aws-execute-load-tests.sh and cleanup.sh with canned values in the form the real
# CLI prints them (`--output text` gives the bare value, otherwise JSON) and keeps no state: every resource the
# runner asks for exists, so the cleanup finds and removes each of them. Names are taken from the arguments, so
# the ARNs and the bucket lines carry the run id of the caller.
#
# Usage: stub-aws.sh <service> <operation> [options...]
# Environment: AWS_STUB_LOG, when set, is a file that receives one line per call (the arguments joined by
#   spaces), so a test can check which calls were made and in which order.
# Exit status: 0 for a known call; 252 (the usage status of the AWS CLI) for any other call.
set -euo pipefail

ACCOUNT_ID="000000000000"
VPC_ID="vpc-0a1b2c3d4e5f60718"
GROUP_ID="sg-0a1b2c3d4e5f60718"
INSTANCE_ID="i-0a1b2c3d4e5f60718"
# The AMI that the public SSM parameter of Canonical names; the stub launches nothing with it.
AMI_ID="ami-0e86e20dae9224db8"

if [[ -n ${AWS_STUB_LOG:-} ]]; then
  printf '%s\n' "$*" >>"${AWS_STUB_LOG}"
fi

service=${1:-}
operation=${2:-}
shift $(($# < 2 ? $# : 2))

# The options of the call, by name; a flag without a value holds the empty string.
output="json" query="" role_name="" policy_name="" profile_name="" bucket=""
while (($# > 0)); do
  case $1 in
    --output) output=${2:-} ;;
    --query) query=${2:-} ;;
    --role-name) role_name=${2:-} ;;
    --policy-name) policy_name=${2:-} ;;
    --instance-profile-name) profile_name=${2:-} ;;
    --bucket) bucket=${2:-} ;;
    s3://*) bucket=${1#s3://} ;;
  esac
  shift
done
bucket=${bucket%%/*}

# answer <text> <json>: the bare value for `--output text`, the JSON document otherwise.
answer() {
  if [[ ${output} == text ]]; then
    printf '%s\n' "$1"
  else
    printf '%s\n' "$2"
  fi
}

# quoted_in_query <field>: the value compared with <field> in a JMESPath filter such as [?Field=='value'].
quoted_in_query() {
  local rest=${query#*"$1"==\'}
  [[ ${rest} != "${query}" ]] || return 0
  printf '%s' "${rest%%\'*}"
}

unsupported() {
  echo "stub-aws.sh: unsupported call: ${service} ${operation}" >&2
  exit 252
}

case "${service} ${operation}" in
  "ssm get-parameter")
    answer "${AMI_ID}" "{\"Parameter\": {\"Type\": \"String\", \"Value\": \"${AMI_ID}\"}}"
    ;;
  "sts get-caller-identity")
    answer "${ACCOUNT_ID}" "{\"Account\": \"${ACCOUNT_ID}\", \"Arn\": \"arn:aws:iam::${ACCOUNT_ID}:user/stub\"}"
    ;;
  "ec2 describe-vpcs")
    answer "${VPC_ID}" "{\"Vpcs\": [{\"VpcId\": \"${VPC_ID}\", \"IsDefault\": true}]}"
    ;;
  "ec2 create-security-group")
    answer "${GROUP_ID}" "{\"GroupId\": \"${GROUP_ID}\"}"
    ;;
  "ec2 describe-security-groups")
    answer "${GROUP_ID}" "{\"SecurityGroups\": [{\"GroupId\": \"${GROUP_ID}\"}]}"
    ;;
  "ec2 run-instances")
    answer "${INSTANCE_ID}" "{\"Instances\": [{\"InstanceId\": \"${INSTANCE_ID}\"}]}"
    ;;
  "ec2 describe-instances")
    answer "${INSTANCE_ID}" "{\"Reservations\": [{\"Instances\": [{\"InstanceId\": \"${INSTANCE_ID}\"}]}]}"
    ;;
  "ec2 terminate-instances")
    answer "${INSTANCE_ID}" "{\"TerminatingInstances\": [{\"InstanceId\": \"${INSTANCE_ID}\"}]}"
    ;;
  "ec2 wait" | "ec2 delete-security-group") ;;
  "s3 mb")
    echo "make_bucket: ${bucket}"
    ;;
  "s3 rb")
    echo "remove_bucket: ${bucket}"
    ;;
  "s3 rm" | "s3api put-bucket-policy") ;;
  "s3api head-bucket")
    answer "us-east-1" "{\"BucketRegion\": \"us-east-1\"}"
    ;;
  "iam create-role" | "iam get-role")
    answer "arn:aws:iam::${ACCOUNT_ID}:role/${role_name}" \
      "{\"Role\": {\"RoleName\": \"${role_name}\", \"Arn\": \"arn:aws:iam::${ACCOUNT_ID}:role/${role_name}\"}}"
    ;;
  "iam create-policy")
    answer "arn:aws:iam::${ACCOUNT_ID}:policy/${policy_name}" \
      "{\"Policy\": {\"PolicyName\": \"${policy_name}\", \"Arn\": \"arn:aws:iam::${ACCOUNT_ID}:policy/${policy_name}\"}}"
    ;;
  "iam list-policies")
    policy_name=$(quoted_in_query PolicyName)
    answer "arn:aws:iam::${ACCOUNT_ID}:policy/${policy_name}" \
      "{\"Policies\": [{\"PolicyName\": \"${policy_name}\", \"Arn\": \"arn:aws:iam::${ACCOUNT_ID}:policy/${policy_name}\"}]}"
    ;;
  "iam create-instance-profile")
    answer "arn:aws:iam::${ACCOUNT_ID}:instance-profile/${profile_name}" \
      "{\"InstanceProfile\": {\"InstanceProfileName\": \"${profile_name}\", \"Arn\": \"arn:aws:iam::${ACCOUNT_ID}:instance-profile/${profile_name}\"}}"
    ;;
  "iam get-instance-profile")
    # With the role filter of cleanup.sh the answer is the role the filter names: the role is attached.
    role_name=$(quoted_in_query RoleName)
    answer "${role_name}" \
      "{\"InstanceProfile\": {\"InstanceProfileName\": \"${profile_name}\", \"Roles\": [{\"RoleName\": \"${role_name}\"}]}}"
    ;;
  "iam attach-role-policy" | "iam add-role-to-instance-profile" | "iam detach-role-policy" | "iam delete-policy" | \
    "iam remove-role-from-instance-profile" | "iam delete-instance-profile" | "iam delete-role") ;;
  *) unsupported ;;
esac
