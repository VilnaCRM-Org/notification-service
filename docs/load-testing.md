# Load testing

The load-test kit is in `tests/Load`. It runs [K6](https://k6.io) from the pinned image `grafana/k6` (nothing is built) against the production image of the service. This document describes the profiles, the budgets and the rule that derives them, how to add a scenario, the isolated environment the runs use, and the runner for a temporary AWS instance.

## Profiles

Each scenario runs up to four profiles. The settings of every profile are declared per endpoint in `tests/Load/config.json.dist`.

| Profile | Shape | Requests per second | Virtual users | Duration | Where it runs |
| --- | --- | --- | --- | --- | --- |
| `smoke` | constant arrival rate | 10 | 5 | 10 seconds | every pull request and every push to `main` (`make smoke-load-tests`) |
| `average` | ramp up, plateau, ramp down | peak 25 | 25 | 3 seconds rise, 10 seconds plateau, 3 seconds fall | on demand (`make average-load-tests`) |
| `stress` | ramp up, plateau, ramp down | peak 150 | 150 | 3 seconds rise, 10 seconds plateau, 3 seconds fall | on demand (`make stress-load-tests`) |
| `spike` | ramp up, ramp down | peak 200 | 200 | 5 seconds rise, 5 seconds fall | on demand (`make spike-load-tests`) |

`make load-tests` runs all four profiles one after the other, with the pause `delayBetweenScenarios` (60 seconds) between them. Only the smoke profile runs in a pipeline. The smoke profile of the one shipped scenario takes about 15 seconds once the environment is up.

A run fails when the p99 of the request duration of a profile exceeds its budget, or when the success rate of its checks is not above 99%. The runners fail with `Error: No load test scenarios found.` when `tests/Load/scripts` holds no scenario.

## Budgets

`threshold` in `tests/Load/config.json.dist` is the p99 budget in milliseconds. It must be a positive number; any other value is rejected with `Invalid threshold value` before the run starts. A local `tests/Load/config.json` (git-ignored) overrides the file, and the variables `API_HOST` and `API_PORT` override the target.

The budgets of the `health` scenario (`GET /api/health`, checked for status 204):

| Profile | Budget (p99, milliseconds) | Ceiling (milliseconds) | Where the budget comes from |
| --- | --- | --- | --- |
| `smoke` | 50 | 500 | the budget rule applied to the p99 measured on a 4-vCPU GitHub-hosted runner (7.25 ms on 2026-11-11) |
| `average` | 200 | 200 | the ceiling: no hosted-runner measurement |
| `stress` | 1000 | 1000 | the ceiling: no hosted-runner measurement |
| `spike` | 5000 | 5000 | the ceiling: no hosted-runner measurement |

Each health request makes one `SELECT 1` and one `ListQueues` call to LocalStack. The smoke profile ran on a 4-vCPU GitHub-hosted runner in the job `Smoke load tests` of commit `db7fe08` (2026-11-11): p99 7.25 ms, 101 of 101 checks passed. The rule gives 3 x 7.25 = 21.75 ms, rounded up to 30 ms, raised to the floor of 50 ms, so the smoke budget is 50 ms. A second hosted run, job `Smoke load tests` of commit `14d0d18` on the same day, measured a p99 of 6.45 ms with 101 of 101 checks: the rule gives 3 x 6.45 = 19.35 ms, rounded up to 20 ms, raised to the floor of 50 ms, so the budget stays 50 ms. CI runs the smoke profile only. The average, stress and spike profiles run in no pipeline (each of them, and `make load-tests`, was run locally on 2026-11-11; see the table below), so they have no hosted-runner measurement and stay at their ceilings; their local measurements are in the table below and are not hosted-runner values. On that machine, while other work shared it, a second stress run measured 1.48 s and failed the 1000 ms budget (load average 9.3), and one smoke run during a concurrent container compile measured 66.41 ms and failed the 50 ms budget (load average 11); every other local run stayed within its budget. The budgets and ceilings were not changed: the rule uses the hosted-runner value, and a busy developer machine can fail `make smoke-load-tests`.

The four budget values are protected configuration: changing one needs the label `approved-config-change` on the pull request. The rest of the file changes freely. The success-rate threshold of the checks (above 99%) lives in `tests/Load/utils/thresholdsBuilder.js`, which is a protected file in its entirety.

### The budget rule

Each budget is three times the p99 measured on a 4-vCPU GitHub-hosted runner, rounded up to a multiple of 10 milliseconds, and it is not below 50 milliseconds (so that runner noise does not fail the gate) and not above its ceiling:

```text
budget = min(ceiling, max(50, 3 x measured p99 rounded up to 10))
```

Ceilings: smoke 500 ms, average 200 ms, stress 1000 ms, spike 5000 ms (the PHP template used 9000 ms for smoke).

Procedure:

1. After the first run of the smoke profile on a hosted runner, read the p99 of the `smoke` profile from the job log or from the `load-test-results` artifact.
2. Compute the budget with the rule and record the measurement, its date and the budget in the table below.
3. Change `threshold` of the smoke profile in `tests/Load/config.json.dist` in a pull request with the label `approved-config-change`.
4. The average, stress and spike profiles run in no pipeline. They stay at their ceilings until a maintainer measures them on comparable hardware (the default AWS instance has 4 vCPU) and applies the same rule.

## Measured values

One row per measurement. Add a row for every new measurement; do not overwrite old rows. Only a hosted-runner row derives a budget; a local row records what the profile measured on that machine. An empty cell means that nothing has been measured yet.

| Scenario | Profile | Environment | Date | Measured p99 (ms) | Budget derived (ms) | Budget in `config.json.dist` (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| `health` | `smoke` | local, 4 vCPU virtual machine, inside `make ci` after a clean clone, production image, `BUILD_IMAGE=0` (details in `docs/operations.md`) | 2026-11-11 | 14.79 | not derived; the rule uses the hosted-runner value | 500 |
| `health` | `smoke` | GitHub-hosted runner, 4 vCPU, `ubuntu-24.04`, job `Smoke load tests` of commit `db7fe08`, production image | 2026-11-11 | 7.25 | 50 (3 x 7.25 = 21.75, rounded up to 30, floor 50) | 50 |
| `health` | `smoke` | GitHub-hosted runner `ubuntu-24.04`, job `Smoke load tests` (job id 130769689140, run 43091554513) of commit `14d0d18`, production image built in the job; that commit still held the budget of 500 ms (101 of 101 checks) | 2026-11-11 | 6.45 | 50 (3 x 6.45 = 19.35, rounded up to 20, floor 50) | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make average-load-tests` (324 requests, checks 100%) | 2026-11-11 | 14.34 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make stress-load-tests` (1949 requests, checks 100%) | 2026-11-11 | 16.71 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make spike-load-tests` (999 requests, checks 100%) | 2026-11-11 | 219.98 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` (all four profiles in one run, 3374 checks, 100%) | 2026-11-11 | 25.37 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 19.13 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 57.23 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 91.77 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs on an idle machine (101 checks each, 100%) | 2026-11-11 | 25.88, 23.51, 35.22 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs with all four cores busy (load average 9 to 12; 101 checks each, 100%) | 2026-11-11 | 24.51, 23.71, 30.43 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs as the machine was (100 checks each, 100%) | 2026-11-11 | 18.9, 22.55, 16.66 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs with four busy loops (100 checks each, 100%) | 2026-11-11 | 33.63, 20.03, 17.66 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests` during a concurrent container compile (load average 11; 100 of 100 checks); **crossed the budget, exit 2** | 2026-11-11 | 66.41 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make average-load-tests` (325 of 325 checks) | 2026-11-11 | 32.04 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make stress-load-tests` (load average 9.3; 1873 of 1873 checks, 76 dropped iterations); **crossed the budget, exit 2** | 2026-11-11 | 1480 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make spike-load-tests` (999 of 999 checks) | 2026-11-11 | 144.85 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` (all four profiles in one run, 3374 of 3374 checks) | 2026-11-11 | 38.86 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 35.63 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 399.22 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 394.73 | not derived; the rule uses the hosted-runner value | 5000 |

## Running the tests

| Target | What it does |
| --- | --- |
| `make smoke-load-tests` | every scenario, smoke profile |
| `make average-load-tests` | every scenario, average profile |
| `make stress-load-tests` | every scenario, stress profile |
| `make spike-load-tests` | every scenario, spike profile |
| `make load-tests` | every scenario, all four profiles |
| `make execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false` | one scenario; every profile runs unless its switch (`runSmoke`, `runAverage`, `runStress`, `runSpike`) is set to `false`, so this example runs the smoke profile only; the selected profiles run one after the other with the pause `delayBetweenScenarios` between them, also when a profile in the middle is switched off |
| `make lint-load-tests` | ESLint and Prettier on the load-test JavaScript |

Every run writes one self-contained HTML dashboard to `tests/Load/results/<prefix><scenario>.html` and prints K6's end-of-run summary with the statistics `avg,min,med,max,p(95),p(99)`. The prefixes are `smoke-`, `average-`, `stress-`, `spike-` and `all-`; a run of `make execute-load-tests-script` has none. The directory is git-ignored apart from its `.gitignore`. CI uploads it as the artifact `load-test-results` (14 days).

The scenario script `execute-load-test.sh` in `tests/Load` is the single entry point of every runner. It exits with status 2 and prints its usage when the scenario is empty or unknown.

## Adding a scenario

A scenario is one script and one configuration entry; no runner needs an edit.

1. Create `tests/Load/scripts/<scenario>.js`. Copy `tests/Load/scripts/health.js` and change the request and the checks. The script builds its options from the configuration through the helpers in `tests/Load/utils`.
2. Add `endpoints.<scenario>` to `tests/Load/config.json.dist` with the four profiles (`smoke`, `average`, `stress`, `spike`), each with `threshold`, `rps`, `vus` and `duration`, in the shapes of the table above.
3. Start with ceilings you can defend, then derive the budgets with the budget rule once a measurement exists.
4. Run `make execute-load-tests-script scenario=<scenario> runAverage=false runStress=false runSpike=false` (the smoke profile only), then `make smoke-load-tests`, which now includes the scenario.
5. Run `make lint-load-tests`. The JavaScript runs in K6, not in Node: only the K6 globals (`__ENV`, `__VU`, `__ITER`, `open`) exist, and `console` is forbidden.
6. Add the measurement to the table above.

Only the four budget values of the shipped configuration are protected in `config.json.dist`, and `utils/thresholdsBuilder.js` is protected as a file: run `make validate-configuration` when you change either.

The assistant skill `load-testing` shows the same steps with the example resource as its illustration.

## The isolated environment

Schemathesis and K6 run against the production image in a compose project of their own, `<project>-load`, never against the development stack. The project has its own network and volumes, publishes no database or LocalStack port, and publishes the service on `LOAD_TEST_APP_PORT` (default `18081`). The development service on port 8081 is a debug build and keeps running next to it.

`scripts/with-load-test-stack.sh` builds the production image (unless `BUILD_IMAGE=0`), starts the project and waits until it is healthy, prepares the database, runs the given command with `API_HOST=localhost` and `API_PORT=$LOAD_TEST_APP_PORT`, and always removes the project with its volumes, also when the command fails. When the command fails (a crossed K6 threshold, a failed Schemathesis check), it prints a line with the exit status and the logs of the project, the service's included, to standard error before it removes the project, and exits with the status of the command. It exits with status 2 and prints the logs when the environment does not become healthy.

To test a service that already runs elsewhere, call the runners directly with `API_HOST` and `API_PORT` set; no environment is started then.

## The AWS runner

`make aws-load-tests` runs the load tests on a temporary EC2 instance and `make aws-load-tests-cleanup` removes everything it created. Use it to measure on hardware that is not your own. The host needs Docker and git only: the AWS CLI runs from a pinned image, the run id comes from `/dev/urandom`, and the policy files are rendered with `sed`.

```sh
make aws-load-tests region=eu-west-1 instance_type=c6i.xlarge load_target=smoke-load-tests
make aws-load-tests-cleanup
```

Parameters (all optional): `region`, `ami`, `instance_type`, `instance_tag`, `role`, `branch`, `repo_url`, `security_group`, `load_target`. Defaults: region `us-east-1`; instance type `c6i.xlarge` (4 vCPU, 8 GiB; a cold release build of the image takes about 5 minutes, which a `t2.micro` cannot do); the AMI is Ubuntu 24.04, resolved for the chosen region from the public SSM parameter `/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id`; the repository is `repo_url` (an http or https URL; credentials are removed) or else the `origin` remote of the checkout, converted to HTTPS (a remote that is a local path is refused with exit status 2: give `repo_url=https://…`); the branch is the checked-out branch, which must be pushed; the load target is `smoke-load-tests`.

### Credentials

The runner reads the AWS credentials from the environment of the host and never from a file of the repository:

| Variable | Meaning |
| --- | --- |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | the access key of a user or role allowed to manage the resources listed below |
| `AWS_SESSION_TOKEN` | only with temporary credentials |
| `AWS_REGION` | the region, when the parameter `region` (option `-r`) is not given |

The region is taken from the `region` Make parameter (option `-r`) first, then from the `AWS_REGION` variable of the caller's environment, then `us-east-1`; a value in `.env` is not read for it. The cleanup uses the region recorded in `tests/Load/aws-run.env`, whatever the parameter or the environment say.

Without them (and without local mode) the script stops and names the variables. Export them in the shell, or let a profile tool export them; do not put them in `.env`. The identity needs permissions for EC2 (run and terminate instances, security groups), S3 (bucket, objects, bucket policy), IAM (role, instance profile, managed policy) and the SSM parameter read; use a sandbox account.

### What it does

1. Creates a security group, an S3 bucket, a role with a bucket policy, a managed policy rendered from `s3-access-policy.json`, and an instance profile. Their names carry a run id of 8 hexadecimal characters and are written to `tests/Load/aws-run.env`, which `make aws-load-tests-cleanup` reads. IAM changes need time to propagate: the script retries up to 53 times, 5 seconds apart.
2. Starts the instance with a 30 GB root volume and the shutdown behaviour `terminate`. On the instance, `user-data.sh` installs Docker, git, make and the AWS CLI from Ubuntu packages and snap, clones the repository and branch, runs `make <load_target>`, and in an exit trap uploads `tests/Load/results/` and the cloud-init log, writes `DONE` or `FAILED` to the bucket and shuts the instance down, so it terminates on success and on failure.
3. Waits for `<run-id>/DONE` or `<run-id>/FAILED`, polling every 30 seconds for at most 45 minutes (`wait=false` skips the wait), downloads the results to `tests/Load/results/aws/<run-id>/`, prints `Launched instance: <id>` and `You can access the S3 bucket here: <console URL>`, and exits with status 1 on `FAILED` or on timeout.

Run `make aws-load-tests-cleanup` afterwards, also after a failure or a timeout. It skips resources that no longer exist and removes `aws-run.env`.

### Costs

A smoke run keeps one `c6i.xlarge` instance for the image build (about 5 minutes), the environment start and the profiles, in all roughly 15 minutes of instance time, plus a 30 GB volume and a few megabytes in S3. At the on-demand price of that instance type, which is of the order of 0.15 to 0.20 US dollars per hour in `us-east-1` and differs by region, a run costs a few cents. A run whose `FAILED` marker or timeout leaves the instance up costs by the hour until it is terminated: check the console after a timeout. Prices change; look up the current price of the instance type in your region before you run `average`, `stress` or `spike` profiles for a long time. The security group, bucket and role cost next to nothing, but the cleanup target removes them so that nothing is left behind.

### What is verified

The AWS flow is verified only against the LocalStack dry run: with `LOCAL_MODE_ENV=true` every call goes to the LocalStack container on `LOCALSTACK_PORT` with the placeholder credentials `test`, the AMI is a fixed placeholder, and the wait is skipped because the emulator does not execute user data. The Bats suite asserts the two output markers and a clean `make aws-load-tests-cleanup` in that mode. Nothing in CI starts a real instance. The first real run is yours: expect to adjust the permissions of the identity, the instance type or the region, and report what you had to change.
