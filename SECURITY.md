# Security Policy

## Supported versions

The latest release is the supported version. Fixes are made on `main` and published in a new release; older releases do not receive patches.

## Reporting a vulnerability

Report a vulnerability through GitHub private vulnerability reporting: open the "Security" tab of the repository and choose "Report a vulnerability". Do not open a public issue, pull request or discussion for a vulnerability.

Include the affected version or commit, the steps to reproduce, the impact you expect and, if you have one, a proposed fix.

## Response

A report is acknowledged within 5 working days. The maintainers then assess it, keep you informed of progress and agree a disclosure date with you.

## Advisories

Confirmed vulnerabilities are published as GitHub security advisories, with the affected and fixed versions and credit to the reporter unless you prefer to stay anonymous.

## Scope

In scope: the code, configuration and tooling of this template (the workspace, `xtask`, the Make targets and scripts, the container definitions and the workflows).

Out of scope: services created from the template. A vulnerability in a downstream service is reported to the maintainers of that service. A weakness that comes from the template and affects downstream services is in scope here.

## What the automated gates cover

The repository checks itself on every pull request and on a schedule:

- known advisories, licences, banned crates and sources of the locked dependencies;
- secrets committed to the repository and vulnerabilities in the container images;
- static security analysis of the code and of the workflow files;
- dependency updates proposed by Dependabot.

The JavaScript lock file of the load-test lint tooling, `tests/Load/pnpm-lock.yaml`, is watched by Dependabot only: no gate audits it.

These gates reduce risk but do not replace a review. A clean run is not proof that the template has no vulnerability.
