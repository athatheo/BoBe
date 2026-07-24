import fs from "node:fs";
import { randomUUID } from "node:crypto";
import WebSocket from "ws";

const bodyUrl = required("BOBE_SIM_BODY_URL");
const adapterUrl = required("BOBE_SIM_ADAPTER_URL");
const adapterToken = required("BOBE_SIM_ADAPTER_TOKEN");
const mainUrl = required("BOBE_SIM_MAIN_URL");
const mainToken = required("BOBE_SIM_MAIN_TOKEN");
const deviceId = required("BOBE_SIM_DEVICE_ID");
const ca = fs.readFileSync(required("BOBE_SIM_CA_PATH"));
const cert = fs.readFileSync(required("BOBE_SIM_CERT_PATH"));
const key = fs.readFileSync(required("BOBE_SIM_KEY_PATH"));
const rogueCert = fs.readFileSync(required("BOBE_SIM_ROGUE_CERT_PATH"));
const rogueKey = fs.readFileSync(required("BOBE_SIM_ROGUE_KEY_PATH"));
const transcriptMarker = required("BOBE_SIM_TRANSCRIPT");
const timeoutMs = Number(process.env.BOBE_SIM_TIMEOUT_MS ?? "120000");
const useExternalAdapter = process.env.BOBE_SIM_EXTERNAL_ADAPTER === "1";
const microphonePcm = useExternalAdapter
  ? readWavePcm(required("BOBE_SIM_WAV_PATH"))
  : Buffer.alloc(640 * 5);

const state = {
  route: null,
  playbackStreamId: 0,
  playedSamples: 0,
  speakerFrames: 0,
  speakerFramesThisTurn: 0,
  opusFrames: 0,
  microphoneFrames: 0,
  captions: [],
  phases: [],
  canceling: true,
  cancelVerified: false,
  drainAdmissionDenied: false,
  bodyFramesInFlight: 0,
  maxBodyFramesInFlight: 0,
  completedRoutes: [],
  adapterTerminals: [],
  currentRouteEvidence: null,
  routeEvidence: [],
};
const bodyDone = deferred();
const bodyRegistered = deferred();
const sse = await startSseCapture(mainUrl, mainToken);
await expectTlsRejection(bodyUrl, {
  ca,
  cert: rogueCert,
  key: rogueKey,
  rejectUnauthorized: true,
});
await expectProtocolMinorRejection();

let adapter = null;
if (!useExternalAdapter) {
adapter = await openSocket(
  adapterUrl,
  "bobe.speech-adapter.v1",
  {
    headers: { Authorization: `Bearer ${adapterToken}` },
    maxPayload: 1024 * 1024,
  },
);
const adapterReady = deferred();

adapter.on("message", async (data, isBinary) => {
  try {
    if (isBinary) {
      const { header, payload } = parseAdapterFrame(Buffer.from(data));
      assertAdapterHeader(header);
      const evidence = currentEvidence();
      if (header.kind === 1) {
        if (header.streamId !== evidence.captureStreamId) {
          throw new Error("adapter microphone frame used the wrong capture stream");
        }
        assertNextSequence(
          evidence.adapterMicrophoneSequences,
          header.sequence,
          "adapter microphone",
        );
        state.microphoneFrames += 1;
        return;
      }
      if (header.kind !== 2) {
        throw new Error(`unexpected adapter media kind ${header.kind}`);
      }
      if (evidence.playbackStreamId === 0) {
        evidence.playbackStreamId = header.streamId;
      } else if (header.streamId !== evidence.playbackStreamId) {
        throw new Error("adapter Opus frame used the wrong playback stream");
      }
      assertNextSequence(
        evidence.adapterOpusSequences,
        header.sequence,
        "adapter Opus",
      );
      state.opusFrames += 1;
      const pcm = Buffer.alloc(960);
      await sendBinary(adapter, encodeAdapterFrame({
        ...header,
        kind: 3,
      }, pcm));
      return;
    }
    const message = JSON.parse(data.toString());
    switch (message.type) {
      case "adapter.welcome":
        if (message.protocol_major !== 1 || message.protocol_minor !== 1) {
          throw new Error("adapter welcome version mismatch");
        }
        adapterReady.resolve();
        break;
      case "capture.open":
        if (
          message.codec !== "pcm_s16le"
          || message.sample_rate_hz !== 16000
          || message.channels !== 1
          || message.frame_duration_ms !== 20
        ) {
          throw new Error("adapter capture format mismatch");
        }
        state.route = message.route;
        bindCurrentRoute(message.route);
        currentEvidence().adapterCaptureOpened = true;
        break;
      case "capture.close":
        assertRoute(message.route);
        currentEvidence().adapterCaptureClosed = true;
        await sendJson(adapter, {
          type: "transcript.final",
          connection_generation: state.route.connection_generation,
          lease_id: state.route.lease_id,
          text: transcriptMarker,
        });
        break;
      case "turn.cancel":
        if (message.reason === "capture_canceled_before_open") {
          break;
        }
        bodyDone.reject(new Error(`turn canceled: ${message.reason}`));
        break;
      case "turn.complete":
        {
          const key = routeKey(message.route);
          const evidence = evidenceForRoute(message.route);
          evidence.adapterTerminalCount += 1;
          state.adapterTerminals.push(key);
        }
        break;
      default:
        throw new Error(`unexpected adapter control ${message.type}`);
    }
  } catch (error) {
    bodyDone.reject(error);
  }
});
adapter.on("error", bodyDone.reject);
await sendJson(adapter, {
  type: "adapter.hello",
  protocol_major: 1,
  protocol_minor: 1,
  capture_format: {
    codec: "pcm_s16le",
    sample_rate_hz: 16000,
    channels: 1,
    frame_duration_ms: 20,
  },
  tts_format: {
    codec: "opus",
    sample_rate_hz: 24000,
    channels: 1,
    frame_duration_ms: 20,
  },
  speaker_format: {
    codec: "pcm_s16le",
    sample_rate_hz: 24000,
    channels: 1,
    frame_duration_ms: 20,
  },
});
await withTimeout(adapterReady.promise, timeoutMs, "adapter registration");
}

const body = await openSocket(
  bodyUrl,
  "bobe.body.v1",
  {
    ca,
    cert,
    key,
    rejectUnauthorized: true,
    maxPayload: 1024 * 1024,
  },
);

body.on("message", async (data, isBinary) => {
  try {
    if (isBinary) {
      state.bodyFramesInFlight += 1;
      state.maxBodyFramesInFlight = Math.max(
        state.maxBodyFramesInFlight,
        state.bodyFramesInFlight,
      );
      const frame = parseBodySpeakerFrame(Buffer.from(data));
      const evidence = currentEvidence();
      if (
        frame.streamId !== state.playbackStreamId
        || frame.streamId !== evidence.playbackStreamId
      ) {
        throw new Error("speaker frame used the wrong playback stream");
      }
      assertNextSequence(
        evidence.speakerSequences,
        frame.sequence,
        "body speaker",
      );
      state.speakerFrames += 1;
      state.speakerFramesThisTurn += 1;
      state.playedSamples += 480;
      const acknowledgement = {
        type: "audio.playback.ack",
        protocol_major: 1,
        lease_id: state.route.lease_id,
        turn_id: state.route.turn_id,
        stream_id: state.playbackStreamId,
        played_samples: state.playedSamples,
        credit_ms: 120,
      };
      await sendJson(body, acknowledgement);
      if (state.speakerFrames === 1) {
        await sendJson(body, acknowledgement);
      }
      state.bodyFramesInFlight -= 1;
      return;
    }
    const message = JSON.parse(data.toString());
    switch (message.type) {
      case "welcome":
        bodyRegistered.resolve();
        await sendJson(body, {
          type: "capture.request",
          protocol_major: 1,
          request_id: "bodylink-cancel-request",
          stream_id: 174321,
          trigger: "action_button",
          codec: "pcm_s16le",
          sample_rate_hz: 16000,
          channels: 1,
          frame_duration_ms: 20,
        });
        await sendJson(body, {
          type: "capture.cancel",
          protocol_major: 1,
          request_id: "bodylink-cancel-request",
          stream_id: 174321,
        });
        break;
      case "capture.granted":
        state.route = {
          device_id: deviceId,
          connection_generation: message.connection_generation ?? 0,
          body_session_id: message.body_session_id ?? "00000000-0000-0000-0000-000000000000",
          lease_id: message.lease_id,
          turn_id: message.turn_id,
          capture_stream_id: message.stream_id,
        };
        // Welcome owns connection/session identity. Fill those from the body-side
        // values cached below before sending routed controls.
        state.route.connection_generation = connectionGeneration;
        state.route.body_session_id = bodySessionId;
        if (message.request_id === "bodylink-cancel-request") {
          break;
        }
        bindCurrentRoute(state.route);
        await sendJson(body, {
          type: "audio.capture.open",
          protocol_major: 1,
          lease_id: state.route.lease_id,
          turn_id: state.route.turn_id,
          stream_id: state.route.capture_stream_id,
          codec: "pcm_s16le",
          sample_rate_hz: 16000,
          channels: 1,
          frame_duration_ms: 20,
        });
        const frameCount = Math.ceil(microphonePcm.length / 640);
        for (let sequence = 0; sequence < frameCount; sequence += 1) {
          const pcm = Buffer.alloc(640);
          microphonePcm.copy(
            pcm,
            0,
            sequence * 640,
            Math.min((sequence + 1) * 640, microphonePcm.length),
          );
          await sendBinary(
            body,
            encodeBodyMicrophoneFrame(
              state.route.capture_stream_id,
              sequence,
              pcm,
            ),
          );
          if (useExternalAdapter) {
            state.microphoneFrames += 1;
            await new Promise((resolve) => setTimeout(resolve, 20));
          }
        }
        await sendJson(body, {
          type: "audio.capture.close",
          protocol_major: 1,
          lease_id: state.route.lease_id,
          turn_id: state.route.turn_id,
          stream_id: state.route.capture_stream_id,
          reason: "button_released",
        });
        break;
      case "capture.denied":
        if (message.request_id === "bodylink-cancel-request") {
          state.canceling = false;
          state.cancelVerified = true;
          await requestActualCapture();
          break;
        }
        bodyDone.reject(new Error(`capture denied: ${message.reason}`));
        break;
      case "audio.playback.open":
        assertLease(message);
        if (
          currentEvidence().playbackStreamId !== 0
          && currentEvidence().playbackStreamId !== message.stream_id
        ) {
          throw new Error("body and adapter used different playback streams");
        }
        currentEvidence().playbackStreamId = message.stream_id;
        state.playbackStreamId = message.stream_id;
        state.playedSamples = 0;
        state.speakerFramesThisTurn = 0;
        await sendJson(body, {
          type: "audio.playback.ready",
          protocol_major: 1,
          lease_id: state.route.lease_id,
          turn_id: state.route.turn_id,
          stream_id: state.playbackStreamId,
          credit_ms: 120,
        });
        break;
      case "audio.playback.close":
        assertLease(message);
        if (message.stream_id !== currentEvidence().playbackStreamId) {
          throw new Error("playback close used the wrong stream");
        }
        currentEvidence().playbackCloseCount += 1;
        currentEvidence().playbackCloseStreamId = message.stream_id;
        if (state.speakerFramesThisTurn === 0) {
          throw new Error("playback closed without speaker frames");
        }
        if (state.completedRoutes.length === 0 && !state.drainAdmissionDenied) {
          await expectMainTurnDeniedDuringDrain();
          state.drainAdmissionDenied = true;
          await sendPlaybackDrained();
        } else {
          await sendPlaybackDrained();
        }
        break;
      case "face.set":
        assertLease(message);
        if (message.caption) {
          state.captions.push(message.caption);
          currentEvidence().captions.push(message.caption);
        }
        break;
      case "turn.state":
        assertLease(message);
        state.phases.push(message.state);
        if (!state.canceling) {
          currentEvidence().phases.push(message.state);
        }
        if (message.state === "idle") {
          if (state.canceling) {
            state.canceling = false;
            state.cancelVerified = true;
            state.route = null;
            await requestActualCapture();
          } else {
            const evidence = currentEvidence();
            bindCurrentRoute(state.route);
            state.completedRoutes.push(evidence.routeKey);
            state.routeEvidence.push(evidence);
            state.currentRouteEvidence = null;
            if (state.completedRoutes.length < 2) {
              state.route = null;
              state.playbackStreamId = 0;
              await requestActualCapture();
            } else {
              bodyDone.resolve();
            }
          }
        }
        break;
      case "pong":
        break;
      case "error":
        bodyDone.reject(new Error(`${message.code}: ${message.message}`));
        break;
      default:
        throw new Error(`unexpected body control ${message.type}`);
    }
  } catch (error) {
    bodyDone.reject(error);
  }
});
body.on("error", bodyDone.reject);

let connectionGeneration = 0;
let bodySessionId = "";
body.on("message", (data, isBinary) => {
  if (isBinary) return;
  try {
    const message = JSON.parse(data.toString());
    if (message.type === "welcome") {
      connectionGeneration = message.connection_generation;
      bodySessionId = message.session_id;
    }
  } catch {
    // The primary handler reports malformed controls.
  }
});

await sendJson(body, {
  type: "hello",
  protocol_major: 1,
  protocol_minor: 1,
  device_id: deviceId,
  boot_id: "0123456789abcdef0123456789abcdef",
  firmware: "sim-1.0.0",
  hardware: "bodylink-simulator",
});
await withTimeout(bodyRegistered.promise, 5000, "body registration");
await expectDuplicateBodyRejection();

await withTimeout(bodyDone.promise, timeoutMs, "BodyLink turn");
await new Promise((resolve) => setTimeout(resolve, 200));
const snapshotHeaders = new Headers();
snapshotHeaders.set("Authorization", ["Bearer", mainToken].join(" "));
const snapshotResponse = await fetch(
  `${mainUrl}/conversation/current`,
  { headers: snapshotHeaders },
);
if (!snapshotResponse.ok) {
  throw new Error(`conversation resync failed: HTTP ${snapshotResponse.status}`);
}
const conversation = await snapshotResponse.json();
const userTurns = conversation.turns.filter((turn) => turn.role === "user");
const assistantTurns = conversation.turns.filter(
  (turn) => turn.role === "assistant" && turn.content.length > 0,
);
if (
  userTurns.length < 2
  || assistantTurns.length < 2
  || (
    !useExternalAdapter
    && userTurns.filter((turn) => turn.content === transcriptMarker).length < 2
  )
  || (
    useExternalAdapter
    && userTurns.filter((turn) => hasTranscriptAnchors(turn.content)).length < 2
  )
) {
  throw new Error("canonical conversation snapshot is missing the body turn");
}
if (
  state.completedRoutes.length !== 2
  || new Set(state.completedRoutes).size !== 2
  || state.routeEvidence.length !== 2
) {
  throw new Error("body routes did not complete with unique correlated evidence");
}
if (
  !useExternalAdapter
  && (
    state.adapterTerminals.length !== 2
    || new Set(state.adapterTerminals).size !== 2
    || state.completedRoutes.some((route) => !state.adapterTerminals.includes(route))
  )
) {
  throw new Error("mock adapter did not receive matching route terminals");
}
assertPerRouteEvidence();
if (!state.cancelVerified) {
  throw new Error("release-before-grant cancellation was not verified");
}
if (!state.drainAdmissionDenied) {
  throw new Error("physical playback did not retain global turn admission");
}
if (state.maxBodyFramesInFlight > 6) {
  throw new Error(
    `playback credit exceeded six frames: ${state.maxBodyFramesInFlight}`,
  );
}
sse.stop();
const ownerSse = sse.snapshot();
if (ownerSse.includes(transcriptMarker)) {
  throw new Error("private body transcript leaked into the legacy SSE stream");
}
for (const turn of assistantTurns) {
  if (turn.content.length > 0 && ownerSse.includes(turn.content)) {
    throw new Error("private body assistant response leaked into the legacy SSE stream");
  }
}
if (
  /"type"\s*:\s*"(?:text_delta|tool_call_start|tool_call_complete)"/.test(ownerSse)
) {
  throw new Error("private body stream event leaked into the legacy SSE stream");
}
if (!ownerSse.includes("conversation_changed")) {
  throw new Error("owner surface did not receive the conversation resync signal");
}
if (
  state.microphoneFrames === 0
  || (!useExternalAdapter && state.opusFrames === 0)
  || state.speakerFrames === 0
) {
  throw new Error("BodyLink turn did not carry audio in both directions");
}
if (!state.captions.some((caption) => caption.trim().length > 0)) {
  throw new Error("BodyLink turn did not route response text to the body");
}
if (!state.phases.includes("thinking") || !state.phases.includes("speaking")) {
  throw new Error(`missing routed phases: ${state.phases.join(",")}`);
}

adapter?.close();
body.close();
console.log(JSON.stringify({
  bodylink_turn: "ok",
  duplicate_route_rejected: "ok",
  minor_zero_rejected: "ok",
  replayed_ack_ignored: "ok",
  max_outstanding_body_frames: state.maxBodyFramesInFlight,
  release_before_grant: "ok",
  immediate_second_route: "ok",
  matching_adapter_terminals: useExternalAdapter ? "not_observed" : "ok",
  playback_holds_turn_admission: "ok",
  microphone_frames: state.microphoneFrames,
  opus_frames: state.opusFrames,
  speaker_frames: state.speakerFrames,
  captions: state.captions.length,
  phases: state.phases,
}));

function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name}`);
  return value;
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function openSocket(url, protocol, options) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url, protocol, options);
    socket.once("open", () => resolve(socket));
    socket.once("error", reject);
  });
}

async function expectDuplicateBodyRejection() {
  const duplicate = await openSocket(
    bodyUrl,
    "bobe.body.v1",
    { ca, cert, key, rejectUnauthorized: true },
  );
  const result = deferred();
  duplicate.once("message", () => {
    result.reject(new Error("duplicate body connection received a welcome"));
  });
  duplicate.once("close", () => result.resolve());
  duplicate.once("error", () => result.resolve());
  try {
    await sendJson(duplicate, {
      type: "hello",
      protocol_major: 1,
      protocol_minor: 1,
      device_id: deviceId,
      boot_id: "fedcba9876543210fedcba9876543210",
      firmware: "sim-duplicate",
      hardware: "bodylink-simulator",
    });
  } catch {
    duplicate.terminate();
    return;
  }
  await withTimeout(result.promise, 5000, "duplicate body rejection");
}

async function expectProtocolMinorRejection() {
  const legacy = await openSocket(
    bodyUrl,
    "bobe.body.v1",
    { ca, cert, key, rejectUnauthorized: true },
  );
  const result = deferred();
  legacy.once("message", () => {
    result.reject(new Error("protocol minor 0 received a welcome"));
  });
  legacy.once("close", () => result.resolve());
  legacy.once("error", () => result.resolve());
  try {
    await sendJson(legacy, {
      type: "hello",
      protocol_major: 1,
      protocol_minor: 0,
      device_id: deviceId,
      boot_id: "00112233445566778899aabbccddeeff",
      firmware: "sim-minor-zero",
      hardware: "bodylink-simulator",
    });
  } catch {
    legacy.terminate();
    return;
  }
  await withTimeout(result.promise, 5000, "protocol minor 0 rejection");
}

async function expectTlsRejection(url, options) {
  await new Promise((resolve, reject) => {
    const socket = new WebSocket(url, "bobe.body.v1", options);
    const timer = setTimeout(() => {
      socket.terminate();
      reject(new Error("rogue client certificate was not rejected promptly"));
    }, 5000);
    socket.once("open", () => {
      clearTimeout(timer);
      socket.terminate();
      reject(new Error("rogue client certificate passed BodyLink pinning"));
    });
    socket.once("error", () => {
      clearTimeout(timer);
      resolve();
    });
    socket.once("close", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

function sendJson(socket, value) {
  return sendBinary(socket, JSON.stringify(value));
}

function sendBinary(socket, value) {
  return new Promise((resolve, reject) => {
    socket.send(value, (error) => (error ? reject(error) : resolve()));
  });
}

async function withTimeout(promise, milliseconds, label) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`${label} timed out`)),
          milliseconds,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

async function startSseCapture(baseUrl, token) {
  const controller = new AbortController();
  const response = await fetch(`${baseUrl}/events`, {
    headers: { Authorization: `Bearer ${token}`, Accept: "text/event-stream" },
    signal: controller.signal,
  });
  if (!response.ok || !response.body) {
    throw new Error(`SSE connection failed: HTTP ${response.status}`);
  }
  let captured = "";
  const reader = response.body.getReader();
  const task = (async () => {
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) return;
        captured += Buffer.from(value).toString("utf8");
      }
    } catch (error) {
      if (error.name !== "AbortError") throw error;
    }
  })();
  task.catch((error) => {
    bodyDone.reject(error);
  });
  return {
    snapshot: () => captured,
    stop: () => controller.abort(),
  };
}

function assertLease(message) {
  if (
    !state.route
    || message.lease_id !== state.route.lease_id
    || message.turn_id !== state.route.turn_id
  ) {
    throw new Error("control used a stale lease or turn");
  }
}

function assertRoute(route) {
  if (
    !state.route
    || route.lease_id !== state.route.lease_id
    || route.turn_id !== state.route.turn_id
    || route.connection_generation !== state.route.connection_generation
  ) {
    throw new Error("adapter control used a stale route");
  }
}

function assertAdapterHeader(header) {
  if (
    !state.route
    || header.connectionGeneration !== BigInt(state.route.connection_generation)
    || header.leaseId !== state.route.lease_id
  ) {
    throw new Error("adapter media used a stale route");
  }
}

function assertNextSequence(sequences, sequence, label) {
  if (sequence !== sequences.length) {
    throw new Error(
      `${label} sequence ${sequence} arrived after ${sequences.length - 1}`,
    );
  }
  sequences.push(sequence);
}

function currentEvidence() {
  if (!state.currentRouteEvidence) {
    throw new Error("route evidence is unavailable");
  }
  return state.currentRouteEvidence;
}

function bindCurrentRoute(route) {
  const evidence = currentEvidence();
  const key = routeKey(route);
  if (evidence.routeKey && evidence.routeKey !== key) {
    throw new Error("one simulated turn observed multiple route identities");
  }
  if (
    evidence.captureStreamId !== 0
    && evidence.captureStreamId !== route.capture_stream_id
  ) {
    throw new Error("route capture stream changed during a turn");
  }
  evidence.routeKey = key;
  evidence.captureStreamId = route.capture_stream_id;
}

function evidenceForRoute(route) {
  const key = routeKey(route);
  if (state.currentRouteEvidence?.routeKey === key) {
    return state.currentRouteEvidence;
  }
  const evidence = state.routeEvidence.find((item) => item.routeKey === key);
  if (!evidence) {
    throw new Error("adapter terminal referenced an unknown route");
  }
  return evidence;
}

function assertPerRouteEvidence() {
  const captureStreams = new Set();
  const playbackStreams = new Set();
  for (const evidence of state.routeEvidence) {
    captureStreams.add(evidence.captureStreamId);
    playbackStreams.add(evidence.playbackStreamId);
    const thinking = evidence.phases.indexOf("thinking");
    const speaking = evidence.phases.indexOf("speaking");
    const idle = evidence.phases.lastIndexOf("idle");
    if (
      evidence.captureStreamId === 0
      || evidence.playbackStreamId === 0
      || evidence.playbackCloseCount !== 1
      || evidence.playbackCloseStreamId !== evidence.playbackStreamId
      || evidence.speakerSequences.length === 0
      || evidence.captions.length === 0
      || thinking < 0
      || speaking <= thinking
      || idle <= speaking
    ) {
      throw new Error(`incomplete route evidence for ${evidence.routeKey}`);
    }
    if (
      !useExternalAdapter
      && (
        !evidence.adapterCaptureOpened
        || !evidence.adapterCaptureClosed
        || evidence.adapterTerminalCount !== 1
        || evidence.adapterMicrophoneSequences.length === 0
        || evidence.adapterOpusSequences.length === 0
      )
    ) {
      throw new Error(`incomplete adapter evidence for ${evidence.routeKey}`);
    }
  }
  if (captureStreams.size !== 2 || playbackStreams.size !== 2) {
    throw new Error("sequential routes reused a capture or playback stream");
  }
}

function routeKey(route) {
  return `${route.connection_generation}:${route.lease_id}:${route.turn_id}`;
}

function hasTranscriptAnchors(text) {
  const words = new Set(
    text
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9\s]/g, " ")
      .split(/\s+/)
      .filter(Boolean),
  );
  return ["blue", "lantern", "ready", "careful", "testing"]
    .every((anchor) => words.has(anchor));
}

async function requestActualCapture() {
  const turnNumber = state.completedRoutes.length + 1;
  state.currentRouteEvidence = {
    routeKey: null,
    captureStreamId: 0,
    playbackStreamId: 0,
    playbackCloseCount: 0,
    playbackCloseStreamId: 0,
    speakerSequences: [],
    adapterMicrophoneSequences: [],
    adapterOpusSequences: [],
    captions: [],
    phases: [],
    adapterCaptureOpened: false,
    adapterCaptureClosed: false,
    adapterTerminalCount: 0,
  };
  await sendJson(body, {
    type: "capture.request",
    protocol_major: 1,
    request_id: `bodylink-e2e-request-${turnNumber}`,
    stream_id: 174321 + turnNumber,
    trigger: "action_button",
    codec: "pcm_s16le",
    sample_rate_hz: 16000,
    channels: 1,
    frame_duration_ms: 20,
  });
}

async function sendPlaybackDrained() {
  await sendJson(body, {
    type: "audio.playback.drained",
    protocol_major: 1,
    lease_id: state.route.lease_id,
    turn_id: state.route.turn_id,
    stream_id: state.playbackStreamId,
  });
}

async function expectMainTurnDeniedDuringDrain() {
  const response = await fetch(`${mainUrl}/message`, {
    method: "POST",
    headers: {
      Authorization: ["Bearer", mainToken].join(" "),
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      content: "This message must be rejected until physical playback drains.",
      request_id: randomUUID(),
    }),
  });
  if (response.status !== 409) {
    const bodyText = await response.text();
    throw new Error(
      `main turn during physical drain returned HTTP ${response.status}: ${bodyText}`,
    );
  }
}

function parseAdapterFrame(data) {
  if (data.length < 40 || data.readUInt8(0) !== 1) {
    throw new Error("invalid adapter frame");
  }
  return {
    header: {
      kind: data.readUInt8(1),
      connectionGeneration: data.readBigUInt64BE(4),
      leaseId: uuidFromBytes(data.subarray(12, 28)),
      streamId: data.readUInt32BE(28),
      sequence: data.readUInt32BE(32),
    },
    payload: data.subarray(40),
  };
}

function encodeAdapterFrame(header, payload) {
  const data = Buffer.alloc(40 + payload.length);
  data.writeUInt8(1, 0);
  data.writeUInt8(header.kind, 1);
  data.writeBigUInt64BE(BigInt(header.connectionGeneration), 4);
  uuidToBytes(header.leaseId).copy(data, 12);
  data.writeUInt32BE(header.streamId, 28);
  data.writeUInt32BE(header.sequence, 32);
  payload.copy(data, 40);
  return data;
}

function encodeBodyMicrophoneFrame(streamId, sequence, pcm) {
  if (pcm.length !== 640) {
    throw new Error("microphone PCM frame must contain 640 bytes");
  }
  const data = Buffer.alloc(20 + 640);
  data.writeUInt8(1, 0);
  data.writeUInt8(1, 1);
  data.writeUInt32BE(streamId, 4);
  data.writeUInt32BE(sequence, 8);
  data.writeBigUInt64BE(BigInt(sequence * 20000), 12);
  pcm.copy(data, 20);
  return data;
}

function parseBodySpeakerFrame(data) {
  if (
    data.length !== 20 + 960
    || data.readUInt8(0) !== 1
    || data.readUInt8(1) !== 2
  ) {
    throw new Error("invalid speaker PCM frame");
  }
  return {
    streamId: data.readUInt32BE(4),
    sequence: data.readUInt32BE(8),
  };
}

function uuidToBytes(uuid) {
  return Buffer.from(uuid.replaceAll("-", ""), "hex");
}

function uuidFromBytes(bytes) {
  const hex = bytes.toString("hex");
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20),
  ].join("-");
}

function readWavePcm(path) {
  const wave = fs.readFileSync(path);
  if (wave.toString("ascii", 0, 4) !== "RIFF" || wave.toString("ascii", 8, 12) !== "WAVE") {
    throw new Error("speech fixture is not a RIFF/WAVE file");
  }
  let offset = 12;
  while (offset + 8 <= wave.length) {
    const kind = wave.toString("ascii", offset, offset + 4);
    const size = wave.readUInt32LE(offset + 4);
    const start = offset + 8;
    if (kind === "fmt ") {
      const format = wave.readUInt16LE(start);
      const channels = wave.readUInt16LE(start + 2);
      const sampleRate = wave.readUInt32LE(start + 4);
      const bits = wave.readUInt16LE(start + 14);
      if (format !== 1 || channels !== 1 || sampleRate !== 16000 || bits !== 16) {
        throw new Error("speech fixture must be PCM16LE mono at 16 kHz");
      }
    } else if (kind === "data") {
      return wave.subarray(start, start + size);
    }
    offset = start + size + (size % 2);
  }
  throw new Error("speech fixture has no data chunk");
}
