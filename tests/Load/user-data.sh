#!/bin/bash
# Runs once on the load-test instance (EC2 user data). aws-execute-load-tests.sh renders the @@NAME@@
# placeholders with sed before it passes the file to run-instances.
#
# The EXIT trap is installed first: whatever fails, it uploads the results and the cloud-init log, writes
# <run id>/DONE or <run id>/FAILED to the bucket and shuts the instance down. The instance was launched with
# the shutdown behaviour `terminate`, so it disappears on success and on failure.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
export HOME=/root

REPO_URL='@@REPO_URL@@'
BRANCH_NAME='@@BRANCH_NAME@@'
LOAD_TARGET='@@LOAD_TARGET@@'
BUCKET_NAME='@@BUCKET_NAME@@'
RUN_ID='@@RUN_ID@@'
export AWS_DEFAULT_REGION='@@REGION@@'

REPO_DIR=/opt/load-test/repository

finish() {
  local code=$?
  local result=DONE
  set +e
  [[ ${code} -eq 0 ]] || result=FAILED
  if [[ -d ${REPO_DIR}/tests/Load/results ]]; then
    aws s3 cp "${REPO_DIR}/tests/Load/results/" "s3://${BUCKET_NAME}/${RUN_ID}/results/" --recursive
  fi
  aws s3 cp /var/log/cloud-init-output.log "s3://${BUCKET_NAME}/${RUN_ID}/cloud-init-output.log"
  echo "${result}" >/tmp/result
  aws s3 cp /tmp/result "s3://${BUCKET_NAME}/${RUN_ID}/${result}"
  shutdown -h now
}
trap finish EXIT

# Ubuntu packages, with the compose v2 plugin; the AWS CLI comes from snap. No unpinned binary downloads.
apt-get update -y
apt-get install -y docker.io docker-compose-v2 docker-buildx git make
# Bounded at 300 seconds: seeding the snaps of a fresh Ubuntu 24.04 image takes well under a minute, so a seed that
# has not loaded by then is stuck. timeout then fails the script and the EXIT trap reports FAILED and shuts the
# instance down instead of leaving it running.
timeout 300 snap wait system seed.loaded
snap install aws-cli --classic

# A cold release build of the image needs more memory than a small instance has.
fallocate -l 2G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo '/swapfile none swap sw 0 0' | tee -a /etc/fstab

git clone --branch "${BRANCH_NAME}" "${REPO_URL}" "${REPO_DIR}"
cd "${REPO_DIR}"

make "${LOAD_TARGET}"
