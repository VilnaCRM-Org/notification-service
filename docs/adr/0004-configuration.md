# 0004. Configuration

Planning decision: AD-07 (configuration model).

## Context

The service is configured only through the environment, and an operator who gets it wrong must see every problem at once, named by variable, without a secret being echoed. Plain clap stops at the first invalid value and names the flag, not the variable. Loading a `.env` file mutates the process environment, which is unsound once threads exist.

## Decision

`clap` derive defines the command line and binds every setting to an environment variable with `hide_env_values`, so help names each variable and never prints a value. Every setting is an optional string, so clap never rejects a value. `Config::from_args` parses and validates all fields, including cross-field rules, and collects every error; each error names the environment variable and never echoes a value. `dotenvy` loads `.env` first, in a synchronous `main`, before the runtime exists; the process environment wins over the file.

A configuration error exits with status 2. `check-config` and `migrate` print one line `<VARIABLE>: <reason>` per problem on stderr. `serve` writes one JSON log entry per problem on stdout, because every line `serve` writes is JSON; for this the log subscriber is installed once, after the configuration has been read. The AWS key pair is validated as a pair and never stored in the configuration value or printed.

## Consequences

Settings are listed twice, in the argument struct and in the configuration value; a unit test asserts that every argument is consumed. The committed `.env` and `.env.test`, the compose files and the Makefile all follow this model. Configured timeouts are not rejected above the product ceilings, because those bound the defaults, not what an operator may set.

Alternatives rejected: typed clap fields with value parsers, the `config` and `figment` crates, and plain-text configuration errors for `serve`.
