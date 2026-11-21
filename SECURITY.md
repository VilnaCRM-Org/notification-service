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

In scope: the code, configuration and tooling of this repository: the service with its event stream (next section), the workspace, `xtask`, the Make targets and scripts, the container definitions and the workflows.

This repository was created from the Rust service template, and the template sync still updates part of it. A weakness that this repository inherited from the template is reported here and also to the maintainers of the template, because it can affect the other services created from it.

## The event stream: what it does not protect

The service delivers the events of one Kafka topic to browsers and other clients over Server-Sent Events and WebSocket, and as the GraphQL subscription `events` over Server-Sent Events on `/api/graphql` (`docs/streaming-protocol.md`). Know these properties before you deploy it.

- **There is no subscriber authorization.** Any client that reaches the service can read the catalog of event types and subscribe to any event type, also to a type that is not in the catalog. No token, session or user identity is checked. The GraphQL subscription has no subscriber authorization either: it reaches the same events as the other two transports, on the same terms. Authorization with a token from the user service, and private topics, are follow-ups.
- **The topic must carry only events that are safe for every client.** Every event a producer writes to the topic (`KAFKA_TOPIC`) can reach every client that subscribes to its type: its envelope `id`, `type`, `occurredAt` and the whole `data` member. Never write personal data, secrets or anything meant for one user or one tenant to the topic. The record key and the record headers are never forwarded.
- **The origin list binds browsers only.** `CORS_ALLOWED_ORIGINS` decides which web pages a browser lets read the answers, and which pages may open a WebSocket. It is not an authorization: a client outside a browser sends any `Origin` header it likes, or none, and a request without `Origin` is never restricted. The GET form of the GraphQL subscription, made for `EventSource`, is a request a browser sends cross-origin without a preflight, and the list decides only whether the page may read the answer; the request can change no state, because the schema has no mutation root and every operation of `/api/graphql` only reads.
- **Every input a client controls is bounded.** At most 100 event types per subscription, per command and per WebSocket connection; 128 characters per type name; a cursor of 64 bits; 16 384 bytes per WebSocket command and 32 768 bytes per WebSocket message or frame (a larger frame ends the connection before its payload is read); 1 000 names in the catalog; `MAX_SUBSCRIBERS` streams per node. A record value larger than `EVENT_MAX_DATA_BYTES` is skipped before it is parsed. What a client can make a node do is bounded too: a replay reads Kafka in pages of 500 records under `EVENT_REPLAY_PERMITS` permits per node and only as fast as its client takes the frames, a WebSocket command never repeats a read, and the node keeps no entry for an event type without a subscriber. The GraphQL form is bounded the same way and further: the POST body within `HTTP_MAX_BODY_BYTES`, and the query string of a GET within the smaller of `HTTP_MAX_BODY_BYTES` and 65 521 bytes (a longer one gets the 414 problem, or, when the request-target exceeds 65 534 bytes on an HTTP/1.1 connection, the bare 414 of the HTTP server, which the service never sees and which carries no CORS header); a document (`query`) of at most 4 096 characters and an operation name (`operationName`) of at most 128, counted in Unicode code points, in the body of a POST, also without the event stream, and in the query string of a GET, else the 422 problem with one `length` violation for each, which names the bound and never the value; a document of depth at most 16 and complexity at most 1 000, where `Event.data` costs 501, so a document selects it at most once and each result holds one copy of the event object; exactly one root field per subscription; 100 type names of at most 128 characters and a 64-bit cursor, as on the other routes; one operation per request, with no batch; and `MAX_SUBSCRIBERS` for every transport together, so GraphQL subscribers take slots from the same count as SSE and WebSocket streams. A slow client affects only itself, and a client that accepts no data is disconnected after `STREAM_KEEP_ALIVE` (on Linux, for HTTP/1.1 connections; an SSE stream on an HTTP/2 connection stalled by its own flow control keeps its slot until its client leaves, see `docs/operations.md`, section "The send bound and HTTP/2").
- **No limit per client.** The service limits streams per node, not per client address, and it does not limit the rate of WebSocket commands. The proxy in front of the service must limit connections per client address (`docs/operations.md`, section "The edge proxy"), or one client can take every slot of a node.
- **No secret in logs or answers.** No log entry, problem document or configuration error holds the value of a variable (`KAFKA_SASL_PASSWORD` above all), event data, a record key or header, or an event type name taken from a request. A rejected record is logged with its offset and the reason only. On `/api/graphql` no log entry, problem or error built by the service holds a value of the request: not the document, a variable, a type name or a cursor. The document errors of async-graphql (a parse error, an unknown field, a value of the wrong type) keep its messages, which may quote parts of the client's own request, such as a source line or a field, argument, variable, fragment or operation name, back to that client, exactly as the JSON transport does; they are returned only in the answer to the request that holds them and never logged.
- **The connection to Kafka.** The service only reads the topic: it produces nothing, joins no consumer group and commits nothing, so its Kafka principal needs read access to the one topic and nothing else. `KAFKA_SECURITY_PROTOCOL` selects plaintext, TLS or SASL (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512); TLS verifies the broker against the public root certificates the client ships. A private certificate authority, client certificates, OAUTHBEARER and AWS MSK IAM are not supported. Use plaintext only inside a network you trust.

## What the automated gates cover

The repository checks itself on every pull request and on a schedule:

- known advisories, licences, banned crates and sources of the locked dependencies;
- secrets committed to the repository and vulnerabilities in the container images;
- static security analysis of the code and of the workflow files;
- dependency updates proposed by Dependabot.

The JavaScript lock file of the load-test lint tooling, `tests/Load/pnpm-lock.yaml`, is watched by Dependabot only: no gate audits it.

These gates reduce risk but do not replace a review. A clean run is not proof that the service has no vulnerability.
