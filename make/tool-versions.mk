# The single list of tool versions and tool images. Protected file (protected-config.toml).
# Every line is `export NAME := value`: recipes and scripts read the values from the environment, and the
# scripts that also run outside Make read this file with that line format.
# Cargo tools: installed by scripts/install-tools.sh (host, development image) and by taiki-e/install-action
# (CI). A version may be raised only to one listed in manifests/<tool>.json of the pinned action commit.
export CARGO_BINSTALL_VERSION        := 1.26.2
export CARGO_BINSTALL_SHA256_X86_64  := 9e0e064f189a9abb9f8df5fb93c3c810ef5cc19e7212d998c7a64c9e4d3527d5
export CARGO_BINSTALL_SHA256_AARCH64 := a235250cd684af9bf1e3a1e4c216b1d36e6c553553d5d789744f27689d81c2d6
export CARGO_CHEF_VERSION            := 0.1.78
export CARGO_NEXTEST_VERSION         := 0.9.151
export CARGO_LLVM_COV_VERSION        := 0.9.1
export CARGO_MUTANTS_VERSION         := 27.1.0
export CARGO_DENY_VERSION            := 0.20.2
export CARGO_SHEAR_VERSION           := 1.14.0
export CARGO_HACK_VERSION            := 0.6.46
export SQLX_CLI_VERSION              := 0.9.0
export CLIPPY_SARIF_VERSION          := 0.8.0
export TAPLO_VERSION                 := 0.10.0
export TYPOS_VERSION                 := 1.52.0
export ZIZMOR_VERSION                := 1.32.0
export WATCHEXEC_VERSION             := 2.8.0
export GRAPHQL_INSPECTOR_VERSION     := 7.0.2
export YAMLLINT_VERSION              := 1.38.0
export BMALPH_VERSION                := 2.11.0
export DEVCONTAINER_CLI_VERSION      := 0.90.0
# cocogitto: the release flow (scripts/release.sh) and its Bats test; the bats-group job installs it through the
# cargo-binstall fallback of the install action, release.yml through its pinned action.
export COCOGITTO_VERSION             := 7.0.0

# Bats (scripts/install-bats.sh): tag and commit SHA.
export BATS_CORE    := v1.14.0 eb7f42f8d608ac693d7a4b67474f6714ea68cfc5
export BATS_SUPPORT := v0.3.0 24a72e14349690bcbf7c151b9d2d1cdd32d36eb1
export BATS_ASSERT  := v2.2.4 f1e9280eaae8f86cbe278a687e6ba755bc802c1a
export BATS_FILE    := v0.4.0 13ad5e2ffcc360281432db3d43a306f7b3667d60

# Tool images: tag and digest (digests read from the registry on 2026-11-10).
export SPECTRAL_IMAGE     := stoplight/spectral:6.17.1@sha256:3b05e6cd9d43375c454cc0c022e91b3285851043b05c3e738342ea3f84cde76d
export OPENAPI_DIFF_IMAGE ?= tufin/oasdiff:v1.37.0@sha256:ff02af37a254b5199b9d72e7bba93dae5837a9119d59459168d1398893b47aea
export SCHEMATHESIS_IMAGE := schemathesis/schemathesis:4.33.1@sha256:ddf8b3c9e0b3ee7edfa65dad4d0c0e05c53fb5d92f99db379d40da01883474c7
export K6_IMAGE           := grafana/k6:2.3.0@sha256:2dd4756ec6e14e62778916e1eb850108e4b499304c02df56760fa0bdb0fba0b8
export STRUCTURIZR_IMAGE  := structurizr/structurizr:2026.10.26@sha256:8aebde645a7cc8d3b9e1e774cd6d75a84ce45d74ae70eb783876182fe7db31a2
export GITLEAKS_IMAGE     := zricethezav/gitleaks:v8.30.1@sha256:c00b6bd0aeb3071cbcb79009cb16a60dd9e0a7c60e2be9ab65d25e6bc8abbb7f
export TRIVY_IMAGE        := aquasec/trivy:0.76.2@sha256:8f6bb947dbc528902e11f2f5be52f7b519f6a39d5d148f41166f2040499768fb
export ACTIONLINT_IMAGE   := rhysd/actionlint:1.7.12@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667
export HADOLINT_IMAGE     := hadolint/hadolint:v2.15.1@sha256:32dac94127fd60b7b7e3fbfc65e1383b9b5e25c9bfd7b8536de7a539fe68a12d
export SHELLCHECK_IMAGE   := koalaman/shellcheck:v0.11.0@sha256:61862eba1fcf09a484ebcc6feea46f1782532571a34ed51fedf90dd25f925a8d
export MARKDOWNLINT_IMAGE := davidanson/markdownlint-cli2:v0.24.0@sha256:70ead7c0352d22d04bbe612cd96b54ec591538d8f4626cafe391bb2ecf3639ca
export NODE_IMAGE         := node:22.23.4-alpine3.24@sha256:0e1754758c2df4cfa3e68d59e2e1b7374bb7fd9ea6328eaaacf15cc934af8202
export PYTHON_IMAGE       := python:3.13-alpine@sha256:1e1f6334529d1fb26a2ef619d7cffb30a7c7d7cc1cc7338ebee283a58d1aeaca
export AWS_CLI_IMAGE      := amazon/aws-cli:2.39.7@sha256:bba262340b0bc7cbf3a8a48612531263b56f4b280c4f036a07816af1b271bda0
# Input of the negative test of the image scan (G10): an old image with known CRITICAL findings.
export VULNERABLE_FIXTURE_IMAGE := alpine:3.10.9@sha256:451eee8bedcb2f029756dc3e9d73bab0e7943c1ac55cff3a4861c52a0fdd3e98
