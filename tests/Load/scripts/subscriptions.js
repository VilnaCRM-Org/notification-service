import { check, sleep } from 'k6';
import exec from 'k6/execution';
import http from 'k6/http';
import { Trend } from 'k6/metrics';
import { setTimeout } from 'k6/timers';
import { WebSocket } from 'k6/websockets';

import ScenarioUtils from '../utils/scenarioUtils.js';
import Utils from '../utils/utils.js';

// Each iteration requests the catalog, then holds one SSE, one GraphQL and one WebSocket subscriber of the type
// that the publisher of the load-test stack writes (`load-test-publisher`, one event every 100 ms) for `hold`
// seconds, so a profile holds `rps` x `hold` subscribers of each kind once it has ramped up (smoke: 10 x 5 = 50 of
// each kind, 150 in all).
const scenarioName = 'subscriptions';
const eventType = 'load.test';
// setup asks the status route once a second, within the setup timeout of the kit (60 s).
const readinessAttempts = 25;

const utils = new Utils();
const scenarioUtils = new ScenarioUtils(utils, scenarioName);
const profiles = utils.getConfig().endpoints[scenarioName];
const baseUrl = utils.getBaseHttpUrl();
const streamUrl = `${baseUrl}/events/stream?types=${eventType}`;
const socketUrl = `${baseUrl.replace(/^http/, 'ws')}/events/ws?types=${eventType}`;
// The helper's base URL already ends in `/api`, so this is `/api/graphql`.
const graphQlUrl = `${baseUrl}/graphql`;
const graphQlSubscription = JSON.stringify({
  query: `subscription { events(types: ["${eventType}"]) { id type data } }`,
});

// From the `sentAtMs` the publisher wrote to the receipt of the event on a WebSocket of this scenario.
const deliveryLatency = new Trend('event_delivery_latency', true);

/**
 * The kit's options plus one delivery threshold per selected profile: the p99 of `event_delivery_latency` stays
 * below the profile's `deliveryThreshold`. The kit's own thresholds (the catalog p99 budget `threshold` and the
 * success rate of the checks) are kept as they are.
 */
function buildOptions() {
  const options = scenarioUtils.getOptions();
  const thresholds = { ...options.thresholds };
  Object.keys(options.scenarios).forEach(profile => {
    const { deliveryThreshold } = profiles[profile];
    if (typeof deliveryThreshold !== 'number' || deliveryThreshold <= 0) {
      throw new Error(`Invalid deliveryThreshold value of the ${profile} profile`);
    }
    thresholds[`event_delivery_latency{scenario:${profile}}`] = [`p(99)<${deliveryThreshold}`];
  });
  return { ...options, thresholds };
}

export const options = buildOptions();

/**
 * Waits until the node is ready and holds at least one event of the publisher in its memory window, so the
 * start of the stack and of the publisher (two JVM starts) stay out of the measured iterations.
 */
export function setup() {
  for (let attempt = 1; attempt <= readinessAttempts; attempt += 1) {
    const response = http.get(`${baseUrl}/events/status`, { timeout: '1s' });
    if (response.status === 200) {
      const status = response.json();
      if (status.state === 'ready' && status.windowLast !== null) {
        return;
      }
    }
    sleep(1);
  }
  throw new Error(
    `the event stream did not report the state ready with an event in its window within ${readinessAttempts} attempts`
  );
}

/**
 * Records the delivery latency of one WebSocket text frame `{"id","type","data":<event object>}`.
 *
 * @param {string} text
 * @returns {boolean} whether the frame is an event of the publisher
 */
function recordDelivery(text) {
  const { data } = JSON.parse(text);
  if (data === undefined || data === null || typeof data.data !== 'object' || data.data === null) {
    return false;
  }
  if (typeof data.data.sentAtMs !== 'number') {
    return false;
  }
  deliveryLatency.add(Date.now() - data.data.sentAtMs);
  return true;
}

/**
 * Holds one WebSocket subscriber for `holdMilliseconds`, then closes it.
 *
 * @param {number} holdMilliseconds
 * @returns {Promise<{opened: boolean, events: number}>}
 */
function holdWebSocket(holdMilliseconds) {
  return new Promise(resolve => {
    const result = { opened: false, events: 0 };
    const socket = new WebSocket(socketUrl);
    const finish = () => resolve(result);
    // The open event follows the server's 101 answer only.
    socket.onopen = () => {
      result.opened = true;
    };
    socket.onmessage = message => {
      if (recordDelivery(message.data)) {
        result.events += 1;
      }
    };
    socket.onerror = finish;
    socket.onclose = finish;
    setTimeout(() => socket.close(), holdMilliseconds);
  });
}

/**
 * Holds one SSE subscriber for `holdSeconds`. A text/event-stream response never ends by itself, so the request
 * ends by its own timeout; the pinned K6 then answers status 0, the error `request timeout` and the body received
 * before the timeout (probed against this route, see docs/load-testing.md). The request carries a test_type of its
 * own instead of the profile's, so its held duration stays out of the catalog p99 budget.
 *
 * @param {number} holdSeconds
 */
function holdSse(holdSeconds) {
  return http.asyncRequest('GET', streamUrl, null, {
    headers: { Accept: 'text/event-stream' },
    timeout: `${holdSeconds}s`,
    tags: { name: 'GET /api/events/stream', test_type: 'held_subscription' },
  });
}

/**
 * Holds one GraphQL subscriber for `holdSeconds`: the subscription `events` over Server-Sent Events, a POST of the
 * GraphQL path that asks for `text/event-stream`. Like the SSE route's, its event stream never ends by itself, so
 * the request ends by its own timeout and the pinned K6 answers with the body received before it; like the SSE
 * request, it carries the test_type `held_subscription`, so its held duration stays out of the catalog p99 budget.
 *
 * @param {number} holdSeconds
 */
function holdGraphQl(holdSeconds) {
  return http.asyncRequest('POST', graphQlUrl, graphQlSubscription, {
    headers: { Accept: 'text/event-stream', 'Content-Type': 'application/json' },
    timeout: `${holdSeconds}s`,
    tags: { name: 'POST /api/graphql (subscription)', test_type: 'held_subscription' },
  });
}

/**
 * Whether one complete block of the GraphQL event stream is a subscription result of the publisher's type: the line
 * `event: next`, then a `data:` line whose result `{"data":{"events":{"id","type","data"}}}` has the type
 * `load.test`. The keep-alive comment `:`, the block `event: complete` (its `data:` line is empty) and a result
 * whose `data` is null (`{"data":null,"errors":[...]}`) are not.
 *
 * @param {string} block the lines of one block, without the blank line that ends it
 * @returns {boolean}
 */
function isSubscriptionResult(block) {
  const lines = block.split('\n');
  const dataLine = lines.find(line => line.startsWith('data: '));
  if (!lines.includes('event: next') || dataLine === undefined) {
    return false;
  }
  const { data } = JSON.parse(dataLine.slice('data: '.length));
  const events = data === null || typeof data !== 'object' ? null : data.events;
  return events !== null && typeof events === 'object' && events.type === eventType;
}

/**
 * Whether a held GraphQL response received at least one subscription result of the publisher's type. Only the
 * blocks ended by a blank line are read: the timeout can cut the last one short.
 *
 * @param {import('k6/http').RefinedResponse<'text'>} response
 * @returns {boolean}
 */
function receivedSubscriptionResult(response) {
  if (typeof response.body !== 'string') {
    return false;
  }
  return response.body.split('\n\n').slice(0, -1).some(isSubscriptionResult);
}

export default async function subscriptions() {
  const { hold } = profiles[exec.scenario.name];

  const catalog = http.get(`${baseUrl}/event-types`);
  utils.checkResponse(catalog, 'catalog is status 200', res => res.status === 200);

  const [socket, stream, subscription] = await Promise.all([
    holdWebSocket(hold * 1000),
    holdSse(hold),
    holdGraphQl(hold),
  ]);
  check(socket, {
    'WebSocket opened (101)': result => result.opened,
    'WebSocket received an event': result => result.events > 0,
  });
  utils.checkResponse(
    stream,
    'held SSE response received an event',
    res => typeof res.body === 'string' && res.body.includes(`\nevent: ${eventType}\n`)
  );
  utils.checkResponse(
    subscription,
    'held GraphQL subscription received an event',
    receivedSubscriptionResult
  );
}
