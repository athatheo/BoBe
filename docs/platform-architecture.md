# BoBe Platform Architecture

> **Status:** Product and architecture direction under active exploration
> **Date:** July 17, 2026
> **Implemented system:** See [BoBe Implementation Architecture](architecture.md)
> **Detailed physical-device design:** See [Physical BoBe](physical-bobe.md). The external WB-12 repository owns the BodyLink media profile; this document remains authoritative for runtime placement, identity, routing, and multi-surface privacy. An opt-in one-device PTT vertical slice is implemented, while product enrollment and physical validation remain incomplete.

## 1. Why this document exists

BoBe began as a local-first personal companion consisting of a native macOS surface and a Rust daemon on the same computer. That architecture gives one user one clearly bounded companion, but it makes the Mac both BoBe's home and the current gateway to the user.

The product idea is becoming broader:

- BoBe may inhabit ESP32 toys, room devices, wearables, phones and future physical bodies.
- Users may want BoBe when their Mac is unavailable.
- Some users will prefer an easy hosted service.
- Other users will require self-hosting and ownership of their companion's data.
- A phone may be a rich surface, a temporary local intelligence layer, or a gateway for a portable device.
- BoBe must remain recognizably one companion rather than becoming a collection of unrelated chat applications.

The difficult question is therefore not merely how an ESP32 connects to the daemon. It is:

> **Where does a personal BoBe live when the companion has many bodies, some users self-host, other users buy a managed product, phones suspend applications, and physical devices must remain useful and secure for years?**

This document records the current reasoning around that question: the desired product shape, the problems it must solve, alternatives considered, proposed architecture, unresolved decisions, risks, and experiments needed before commitments become shipped guarantees.

## 2. Status vocabulary

This document intentionally separates certainty levels.

| Label | Meaning |
|---|---|
| **Current** | Implemented in the repository today |
| **Accepted direction** | A principle this architecture recommends preserving across implementations |
| **Committed constraint** | A compatibility or product requirement that future work must satisfy, although implementation may be incomplete |
| **Proposed** | Recommended design requiring implementation and validation |
| **Prototype candidate** | An option to test before choosing a permanent design |
| **Open decision** | Insufficient evidence exists; requires an architecture decision record or product decision |
| **No-go** | A path rejected because it violates an authority, security or product boundary |

Most of this document is **proposed**. It must not be read as a list of shipped capabilities or promised dates.

## 3. The central product idea

### 3.1 Proposed invariant

> **Each owner has one writable authoritative personal-runtime epoch. That authority owns conversation state, long-term memory, goals, policy, tool authorization and agent sessions. Any number of surfaces may present, sense or act on behalf of the owner, but they do not fork BoBe's authority. Standby, migration and replacement instances are allowed only when fencing prevents concurrent authority.**

In simpler product language:

> **One person, one BoBe, many bodies.**

This is more precise than saying “exactly one process.” During an upgrade, restore test or regional migration, several runtime processes may temporarily exist. Only one may accept authoritative writes or agent turns for a given authority epoch.

### 3.2 Why authority matters

Without an explicit authority boundary, multiple surfaces create difficult contradictions:

- The phone remembers something the home daemon does not.
- Two devices answer simultaneously with different context.
- A toy and desktop both perform the same external action.
- A stale runtime overwrites newer memory after reconnecting.
- Hosted and self-hosted copies diverge into two different companions.
- A device that was sold or revoked retains access to private memories.

A personal companion needs stronger continuity than an ordinary stateless chatbot. “Which BoBe is real?” must always have an answer.

### 3.3 What makes a surface

A surface can provide:

- input, such as text, microphone audio, buttons, camera frames or sensors;
- output, such as text, speech, expressions, light, movement or haptics;
- local deterministic behavior;
- bounded local intelligence;
- cached state for continuity;
- presentation of authoritative runtime state.

A surface does not independently own:

- the canonical memory;
- the canonical conversation history;
- unrestricted tool credentials;
- the companion's authority policy;
- an unrelated agent identity.

## 4. The problems being solved

### 4.1 Decentralization

The existing local architecture has a natural trust boundary: one user account, one Mac, one daemon and one data directory. Selling a toy changes that:

- the device must be claimed by an account or self-hosted installation;
- Wi-Fi must be configured without a keyboard;
- the toy must find its owner's runtime;
- devices need unique identities and revocation;
- the runtime may move or restart;
- users expect the product to work away from the Mac;
- a hosted service introduces many owners without allowing their data to mix;
- the hardware can outlive the original service provider.

The problem is not “make the current daemon multi-user.” The problem is “let many independent personal runtimes be discovered, provisioned and operated safely.”

### 4.2 Convenience versus sovereignty

A hosted product offers:

- QR-code onboarding;
- no server administration;
- reliable remote availability;
- managed updates and backups;
- easier support;
- a viable consumer retail experience.

Self-hosting offers:

- control of personal data and credentials;
- independence from BoBe's continued operation as a company or service;
- local-network latency;
- the possibility of fully local speech and inference;
- a credible end-of-service path for purchased hardware.

These should not become two different BoBes. The desired outcome is one runtime contract with different deployment operators.

### 4.3 Mobile operating systems are not servers

Rust can run on iOS and Android, but mobile lifecycle policy changes what “run” means.

- An iPhone can run a substantial ahead-of-time Rust library while the app is active.
- A normal iPhone application cannot remain a continuously reachable general daemon after suspension.
- Android can run a user-visible connected-device foreground service for eligible use cases, but the process remains stoppable and policy-controlled.
- The current GitHub Copilot SDK architecture starts a separate CLI child process, which is a desktop/server assumption and does not transfer unchanged to mobile sandboxes.

The phone is therefore valuable, but should not become the sole appliance-grade authority unless the runtime and product assumptions change materially.

### 4.4 Constrained physical hardware

An ESP32 is excellent at deterministic real-time responsibilities:

- audio capture/playback;
- wake word and basic voice activity detection;
- display, touch, lights and actuators;
- networking and buffering;
- device identity and secure boot;
- low-power operation.

It is not an appropriate home for BoBe's general agent loop, durable memory, unrestricted tools, Copilot CLI, large speech models or broad credentials. Trying to make it the intelligence authority would dilute BoBe into a weaker embedded chatbot.

### 4.5 Multi-surface consistency

The current event and voice paths assume one presentation client. A real platform must support:

- several connected surfaces;
- targeted versus broadcast events;
- reconnection and replay;
- response routing to the initiating surface;
- one active spoken turn without disconnecting every idle device;
- device provenance on input;
- per-device capabilities and permissions;
- bounded queues and backpressure;
- offline and degraded modes.

### 4.6 Trust and longevity

A physical companion may hear private speech, interact with children, operate actuators and accumulate emotionally meaningful history. Architecture must address:

- microphone transparency and physical mute;
- child privacy and consent;
- deletion and export;
- device resale and ownership transfer;
- credential extraction;
- malicious or abandoned firmware;
- server shutdown;
- compromised dependencies;
- safe tool authorization;
- recovery from a failed update;
- what the toy does when disconnected.

## 5. Product goals

### 5.1 Goals

1. Preserve one continuous BoBe identity across surfaces.
2. Keep durable personal authority out of disposable endpoint hardware.
3. Offer a low-friction hosted product.
4. Preserve a credible self-hosted path using the same owner-facing protocol.
5. Make the existing managed-local Mac deployment remain valid while the platform evolves.
6. Permit remote operation without exposing broad administrative APIs to toys.
7. Make device enrollment, revocation and ownership transfer understandable to consumers.
8. Support graceful disconnection rather than pretending the cloud is always available.
9. Separate product control-plane metadata from private companion content.
10. Give each deployment an auditable security and update boundary.
11. Enable mobile value without depending on unsupported background behavior.
12. Make purchased hardware survivable if the hosted service changes or ends.

### 5.2 Non-goals

1. Running BoBe's full intelligence on an ESP32.
2. Building a separate personality or memory for every surface.
3. Making the current daemon a conventional shared SaaS process.
4. Promising that an iPhone remains an always-on local server.
5. Exposing the existing administrative daemon API directly to a toy.
6. Choosing custom hardware before an off-the-shelf prototype proves the experience.
7. Selecting Kubernetes, microVMs, a message broker, PKI format or billing system before requirements and measurements justify them.
8. Replacing the native macOS product with a generic cross-platform shell.
9. Claiming fully local operation when any speech, inference or tool stage still uses a hosted provider.

## 6. Alternatives considered

### 6.1 Add tenants to one shared daemon

```text
one daemon
  ├── owner A state
  ├── owner B state
  └── owner C state
```

**Why it is attractive:**

- fewer processes;
- simpler-looking deployment;
- potentially lower idle cost;
- conventional SaaS database patterns.

**Why it is not recommended initially:**

- the current `AppState`, runtime session, turn gate, event queue, Copilot client, secret store and SQLite model are process-scoped and single-owner;
- tenant context would need to reach every repository, event, queue, cache, file, tool, secret, model session and log;
- one missed scope check could expose intimate personal data;
- arbitrary tools and MCP processes need per-owner execution boundaries anyway;
- active conversations and proactive behavior are stateful, making request-level stateless pooling misleading;
- it would force SaaS complexity into the self-hosted personal runtime.

**Conclusion:** Rejected for the first hosted architecture. Re-evaluate only with strong isolation tests and evidence that process-per-owner economics are untenable.

### 6.2 One isolated runtime per owner

```text
shared control plane
  ├── runtime A + private volume/secrets
  ├── runtime B + private volume/secrets
  └── runtime C + private volume/secrets
```

**Why it is attractive:**

- closely preserves the current mental model;
- creates an owner-scoped state, lifecycle and fault-containment unit; the actual security boundary still depends on the chosen container or microVM substrate, host hardening, identity separation, filesystem/network policy and threat model;
- allows self-hosted and hosted deployments to use the same runtime artifact;
- separates owner state and process trees;
- makes backup, deletion and restore owner-scoped;
- reduces the amount of tenant-aware code inside the companion.

**Problems to solve:**

- idle resource cost;
- startup/wake latency;
- runtime scheduling and placement;
- upgrades across many instances;
- per-owner observability;
- secure volume and secret lifecycle;
- fencing during migration and recovery;
- capacity limits for model and tool workloads.

**Conclusion:** Recommended starting model.

### 6.3 Make the phone the personal runtime

**Why it is attractive:**

- users already own capable hardware;
- the phone travels with them;
- BLE wearables naturally connect to it;
- local speech and small on-device models improve privacy and latency;
- no dedicated home server is required.

**Why it cannot be the only architecture:**

- iOS suspends normal applications;
- Android's long-lived service model is visible and conditional;
- the current Copilot SDK needs a child CLI process;
- tool execution and arbitrary MCP integrations conflict with mobile sandbox assumptions;
- battery, thermal and storage limits constrain continuous work;
- the toy could become unavailable whenever the phone app is suspended, killed, absent or out of range.

**Conclusion:** Use mobile as a rich surface, local accelerator and optional relay. Keep “portable personal runtime” as a future experiment if the agent runtime becomes embeddable and product expectations remain foreground/opportunistic.

### 6.4 Make the Mac a permanent gateway for toys

**Why it is attractive:**

- much of BoBe already runs there;
- the existing voice path is optimized for Apple Silicon;
- no hosted platform is initially required.

**Why it is insufficient as the product architecture:**

- the toy stops being useful when the Mac sleeps or leaves the network;
- consumers may not own a Mac;
- portable toys need another path;
- it couples hardware value to a desktop application's lifecycle;
- it makes self-hosting on Linux secondary rather than first-class.

**Conclusion:** Keep as a useful deployment and prototype path, not as the universal topology.

### 6.5 Make the ESP32 the complete companion

**Why it is attractive:**

- apparent decentralization;
- no server dependency;
- instant local availability.

**Why it is rejected:**

- insufficient compute and memory for the intended agent, speech and multimodal stack;
- credentials and durable memory would reside on extractable consumer hardware;
- model/tool quality would differ from desktop BoBe;
- updates and data migration become firmware problems;
- each body could become an independent, diverging companion.

**Conclusion:** Explicit no-go. The ESP32 is an embodiment endpoint.

### 6.6 Cloud-only toy service

**Why it is attractive:**

- simplest retail support story;
- centralized updates and quality control;
- predictable routing and model access.

**Why it weakens BoBe:**

- purchased hardware can become unusable after service shutdown;
- user trust depends entirely on the company;
- local-first differentiation disappears;
- privacy and child-safety exposure increases;
- hosted costs become inseparable from hardware operation.

**Conclusion:** Hosted service should be convenient but not define the protocol or make self-hosting impossible.

## 7. Proposed target architecture

```text
                                    Proposed BoBe platform

┌────────────────────────────── Shared control plane ──────────────────────────┐
│ Account login • owner/device registry • claiming • runtime directory         │
│ lifecycle • routing metadata • fleet/OTA metadata • subscriptions • support │
└───────────────────────────────────┬───────────────────────────────────────────┘
                                    │ selects and authorizes
                                    ▼
┌────────────────────── Owner's authoritative runtime epoch ──────────────────┐
│                                                                              │
│ BoBe identity • conversations • memory • goals • policy • agent sessions    │
│ tool authorization • MCP • speech/inference adapters • proactive behavior   │
│                                                                              │
│ private database/files • private secrets • private process/tool boundary     │
└───────────────────────┬────────────────────┬─────────────────────────────────┘
                        │                    │
                owner-facing API     endpoint/gateway API
                        │                    │
          ┌─────────────┼──────────┐         ├───────────────┐
          │             │          │         │               │
      macOS surface  iOS surface Android  Wi-Fi ESP32    BLE ESP32
       Current +      Proposed     Proposed  room toy       wearable
       evolving
```

The diagram is a target view. Only the current macOS surface and personal runtime implementation exist today.

## 8. Deployment model

### 8.1 Managed-local

**Current and preserved unless explicitly reconsidered.**

The Mac application supervises a personal runtime on the same machine. This remains the best high-performance path for native screen awareness and Apple Silicon voice features.

Future platform work should protect it with regression tests. It should not be forced through a hosted control plane for routine local interaction.

### 8.2 Self-hosted personal runtime

**Committed architectural option; proposed supported product.**

The user runs the personal runtime on an Ubuntu x86_64 host, home server or compatible appliance. The deployment needs:

- a supported standalone runtime artifact or container;
- service supervision;
- authenticated TLS or a well-defined trusted reverse proxy boundary;
- durable data and secret directories;
- model/artifact management;
- backup and restore;
- upgrade and rollback;
- health and observability;
- device enrollment independent of BoBe's hosted control plane where practical;
- a stable endpoint discovery or configuration mechanism.

Self-hosting is not “run `cargo run` on another computer.” It is a product and operational contract.

### 8.3 Hosted personal runtime

**Proposed.**

The hosted platform operates many isolated single-owner runtimes.

Minimum boundary per owner:

- private database and data volume;
- private secrets;
- private Copilot CLI/session process tree;
- isolated tool/MCP execution;
- resource and network policy;
- owner-scoped logs and metrics;
- explicit lifecycle and deletion;
- authority epoch and fencing.

A runtime may sleep when inactive if wake latency remains acceptable. The control plane can start it before routing a session, but must surface availability honestly.

### 8.4 Hybrid routing

A client or device should not need a fundamentally different conversation protocol based on who operates the runtime. It should receive or discover an endpoint and authenticate to the same versioned owner-facing or endpoint contract.

Possible deployments:

```text
Hosted:
  surface/device → BoBe edge → selected owner runtime

Self-hosted remote:
  surface/device → owner's domain/VPN/reverse proxy → owner runtime

Local LAN:
  surface/device → discovered owner runtime

Portable BLE:
  endpoint → phone relay → hosted/self-hosted runtime
```

The control plane should not become a mandatory content relay for local or self-hosted communication unless the user explicitly chooses a relay-dependent topology.

## 9. Control plane versus personal runtime

### 9.1 Control-plane responsibilities

The shared control plane may own:

- account authentication;
- owner and installation identifiers;
- device manufacturing records;
- one-time claiming challenges;
- mapping from owner to active runtime authority;
- runtime placement and lifecycle;
- endpoint routing metadata;
- credential issuance and revocation metadata;
- fleet firmware channels and signed manifest metadata;
- subscriptions, quotas and support metadata;
- coarse health and usage accounting;
- encrypted backup inventory metadata.

### 9.2 What the control plane should not own by default

The shared control plane should not be the routine authority for:

- conversation content;
- long-term memory;
- goals and profile content;
- model prompts and tool results;
- provider credentials;
- MCP credentials;
- arbitrary runtime files;
- agent decisions.

Hosted operation may necessarily place encrypted or operational copies in provider infrastructure, but the conceptual ownership remains the personal runtime. Whether relays can be cryptographically blind to content is an open design decision; TLS alone is not end-to-end encryption.

### 9.3 Personal runtime responsibilities

The runtime owns:

- canonical BoBe identity and behavior;
- canonical personal state;
- authoritative conversation and turn ordering;
- long-term memory and goals;
- tool policy and credentials;
- Copilot/model sessions;
- proactive scheduling;
- endpoint capabilities and permissions;
- response routing;
- durable event history or snapshots needed for reconnection;
- reconciliation of bounded local/offline interactions.

## 10. Identity and authority model

Exact certificate and token formats require architecture decision records. The conceptual principals should be stable.

| Principal | Meaning |
|---|---|
| Owner | Human or household authority for one personal BoBe |
| Runtime authority | Stable identity of the owner's personal runtime installation/authority |
| Runtime incarnation | One process/container instance serving an authority epoch |
| Surface | Rich user client such as Mac or phone |
| Endpoint | Constrained physical embodiment such as an ESP32 toy |
| Control plane | Shared service that enrolls, locates and operates runtimes/endpoints |

### 10.1 Required properties

Future identity mechanisms should provide:

- proof of device or installation identity;
- owner binding;
- audience-bound credentials;
- minimum necessary scopes;
- short-lived surface authorization where practical;
- replay resistance;
- revocation;
- ownership transfer;
- authority epochs and fencing;
- credential rotation;
- recovery that does not silently create two authorities.

### 10.2 Stable identity versus incarnation

A runtime needs a stable identity across normal restarts. It also needs an incarnation or authority epoch so stale instances cannot resume writes after replacement.

```text
runtime_authority_id: stable installation/authority
incarnation_id:       one boot/deployment instance
authority_epoch:      monotonic fencing generation
```

The exact representation remains open.

## 11. Device claiming and Wi-Fi provisioning

### 11.1 Desired consumer flow

1. The owner installs the BoBe mobile or desktop application.
2. They sign in to hosted BoBe or choose a self-hosted runtime.
3. They power on an unclaimed toy.
4. The application scans the toy's QR code.
5. The QR supplies a manufacturing identity and one-time claim challenge, not a permanent broad secret.
6. The application connects over authenticated BLE.
7. The toy proves possession of its manufacturing/device key.
8. The owner confirms the device and intended BoBe runtime.
9. The application transfers Wi-Fi and endpoint/bootstrap information.
10. The toy joins the network.
11. It exchanges bootstrap authority for a unique scoped operational identity.
12. The bootstrap session expires or is invalidated.
13. The toy establishes its endpoint session with the owner's runtime.

```text
QR identifies and claims
BLE configures and proves proximity
Wi-Fi carries normal room-device traffic
per-device credentials authenticate every later session
```

### 11.2 Self-hosted claiming

A self-hosted owner should not be required to create a permanent BoBe-hosted account merely to use hardware. Candidate approaches include:

- the self-hosted runtime displays a local claim QR;
- the app transfers a local runtime enrollment challenge over BLE;
- an optional hosted rendezvous service assists discovery but does not retain authority;
- advanced users provision through a local web interface.

The simplest secure flow must be prototyped. “Open” should not mean requiring users to manually paste permanent tokens into firmware.

### 11.3 Ownership transfer and reset

A factory reset must:

- erase Wi-Fi and operational credentials;
- remove cached private state;
- make prior owner authorization unusable;
- return the device to an explicitly visible unclaimed state;
- preserve manufacturing identity needed for legitimate re-enrollment;
- avoid permitting physical theft to expose the previous owner's content.

The old owner should be able to revoke a lost device without physical access.

## 12. ESP32 endpoint architecture

### 12.1 Endpoint responsibilities

An ESP32 endpoint may own:

- microphone and speaker I/O;
- local audio front-end and codec handling;
- wake word and basic VAD where feasible;
- buttons, touch, lights, display and actuators;
- physical microphone mute;
- deterministic expressions and idle behavior;
- bounded input/output buffers;
- connection and battery state;
- device identity;
- secure provisioning;
- firmware verification and OTA;
- limited offline status and local commands.

### 12.2 Runtime responsibilities retained off-device

The endpoint should not own:

- full companion memory;
- canonical conversation history;
- provider or MCP credentials;
- unrestricted tools;
- Copilot CLI/session state;
- open-domain speech recognition as a product assumption;
- the general reasoning loop;
- independent proactive policy.

### 12.3 Why this still feels like BoBe

The physical character does not come from where the model runs. It comes from continuity of:

- voice and expression;
- memory;
- goals;
- timing and proactive behavior;
- relationship history;
- permissions;
- recognizable behavior across bodies.

The endpoint is BoBe's body because the authoritative personal runtime animates it consistently.

### 12.4 Offline behavior

A disconnected toy should never imply that full BoBe is available. It can still:

- show a clear offline expression/state;
- respond to mute, volume and local controls;
- play bounded local sounds;
- perform simple deterministic animations;
- queue a small number of idempotent non-sensitive events;
- reconnect with backoff;
- provide setup/recovery instructions.

Storing raw ambient audio for later upload should not be the default. Any offline queue needs bounds, encryption where sensitive, expiry, sequence identifiers and user-visible semantics.

Detailed board choices, acoustic echo cancellation, camera policy and embodiment behavior remain delegated to [Physical BoBe](physical-bobe.md). The first reference body is now the recovered WonderBoy WB-12 in `espBobeToy`; its BodyLink document owns the implemented minor-1 device wire profile, while this document remains authoritative for runtime placement, routing and multi-surface privacy.

## 13. Device transport

### 13.1 Three separate protocol boundaries

The platform should not collapse every connection into one API.

1. **Owner-facing runtime data plane**  
   Used by Mac and mobile surfaces for conversations, settings, events and approved administration.

2. **Physical endpoint data plane**  
   Used by constrained toys for audio, playback, state, controls, expressions and device health.

3. **Control-plane operations**  
   Used for enrollment, runtime selection, revocation, fleet state and update metadata.

A toy should not receive the broad bearer credential currently used by the Mac settings application.

### 13.2 First endpoint protocol decision — July 17, 2026

The first WB-12 milestone uses one persistent BodyLink WebSocket over authenticated TLS:

- JSON text frames for bounded semantic control;
- binary media frames with protocol, kind, flags, stream, sequence and monotonic timestamp;
- PCM16LE mono, 16 kHz microphone and 24 kHz speaker, 20 ms frames;
- explicit playback credit and played-sample acknowledgements;
- push-to-talk and honest half-duplex;
- no wake word, camera, ambient audio persistence or full-duplex AEC.

PCM is a bring-up profile, not a permanent WAN decision. It costs about 32 KB/s uplink and 48 KB/s downlink and makes board/audio failures directly inspectable. Opus is the next negotiated profile only after measured LAN reliability, latency, CPU and audio quality. WebRTC remains a later remote/mobile option; MQTT is not the primary conversational media path.

The endpoint connects to the active personal runtime's scoped body gateway. In the implemented managed-Mac slice, Swift FluidAudio acts as the correlated STT and Opus-decoding adapter. It is replaceable and has no routing authority: it does not open a second persistent `/voice/stream`, assign turns, or choose an endpoint. The daemon assigns the authoritative turn, invokes the same conversation/agent loop and targets text/audio/face output through the initiating endpoint's immutable route.

The protocol must distinguish stable device identity, boot incarnation, connection generation, request, turn, voice lease and media stream. A body connection does not itself own voice; many bodies and the Mac may remain connected while one expiring turn lease participates in the existing shared admission gate.

### 13.3 MQTT consideration

MQTT 5 may be appropriate later for:

- presence;
- desired/reported state;
- durable low-volume commands;
- acknowledgements;
- fleet telemetry;
- update notifications;
- intermittent connections.

It may coexist with WebSocket voice, but adding two stateful transports increases operational complexity. The first prototype should prove whether one WebSocket plus HTTPS is sufficient.

### 13.4 Why not raw UDP first

UDP can reduce overhead, and comparable projects use it for audio. It also introduces:

- NAT and firewall behavior;
- loss/reordering handling;
- encryption and key management;
- congestion behavior;
- additional observability burden;
- more difficult hosted and self-hosted support.

It should be adopted only if measured latency or network behavior justifies it.

### 13.5 Matter

Matter is a candidate interoperability layer for bounded smart-home discovery and control. It is not a complete conversational media and companion protocol. Matter should not block the first BoBe endpoint.

## 14. Multi-surface event model

The current destructive global SSE queue cannot become the platform contract. A future contract requires:

- endpoint/surface identity;
- source provenance on input;
- target and audience on output;
- ordered event identifiers;
- resumable cursors;
- snapshots when replay is unavailable or compacted;
- idempotent commands;
- request and turn correlation;
- delivery acknowledgement where required;
- bounded queues;
- backpressure and slow-consumer handling;
- compatibility and deprecation policy;
- authorization before subscription;
- explicit private versus broadcast event classes.

### 14.1 Routing examples

- Text response deltas may return only to the initiating surface.
- A conversation-complete event may update all of the owner's active surfaces.
- Spoken TTS should normally route to the endpoint holding the voice-turn lease.
- Sensitive tool results should never be broadcast globally.
- Presence events should not imply authorization to receive conversation content.
- Settings changes may target capable administrative surfaces only.

### 14.2 Voice lease

The current global “one voice socket” flag should eventually become an explicit lease:

- many endpoints may remain connected and idle;
- one endpoint holds the active spoken-turn lease;
- barge-in and cancellation affect the owning turn;
- the runtime can reject, queue or transfer new spoken turns according to policy;
- stale leases expire after disconnect/timeouts;
- the lease does not supersede the broader shared agent-turn admission rule.

## 15. Mobile strategy

### 15.1 iPhone role

An iPhone application is strongly valuable as:

- account login and device claiming;
- QR scanner and BLE provisioner;
- portable chat and voice surface;
- notifications and proactive re-entry;
- settings and device administration;
- foreground LAN or remote runtime client;
- opportunistic BLE relay for wearables;
- local speech recognition and synthesis;
- local intent classification, summarization and bounded offline response;
- encrypted cache of recent authorized state.

It should not be required to remain a general background daemon.

### 15.2 Android role

Android can provide the same rich surface and provisioning behavior. It may additionally run a user-visible connected-device foreground service when the use case and store policy permit it.

This can improve BLE relay continuity, but must not be marketed as invisible or guaranteed 24/7 service. Product behavior must survive:

- user stopping the service;
- process death;
- reboot;
- Doze;
- permission revocation;
- manufacturer battery policy;
- loss of the persistent notification privilege.

### 15.3 Can Rust run on mobile?

Yes, in a bounded sense:

- Rust supports iOS and Android targets.
- Tokio supports both platforms.
- iOS can link an ahead-of-time Rust static library/XCFramework through a C ABI.
- Android can load a Rust shared library through JNI.
- SQLite and a meaningful domain core can run in-process.

But the current BoBe runtime cannot move unchanged:

- the GitHub Copilot SDK starts a separate signed CLI helper process;
- current login and process handling assume desktop/server facilities;
- mobile sandbox and executable-code rules differ;
- broad filesystem, shell and MCP tools need new permission and isolation designs;
- lifecycle suspension conflicts with runtime authority.

### 15.4 Potential mobile-local intelligence

On supported Apple hardware, the Foundation Models framework and system speech APIs can provide bounded on-device work such as:

- extraction;
- classification;
- summarization;
- rewriting;
- local tool selection inside app permissions;
- speech transcription;
- speech synthesis.

App-owned Core ML or MLX models are another candidate, with explicit download, storage, memory, thermal and licensing costs. Android has analogous system and app-owned model paths, with more device fragmentation.

These capabilities should augment BoBe rather than silently creating a second authority. Local outputs need provenance and reconciliation rules.

### 15.5 Foreground local-runtime experiment

A future prototype could extract a mobile-safe Rust subset containing:

- domain types;
- encrypted local cache;
- event log/reconciliation;
- deterministic policy;
- retrieval and bounded memory processing;
- a provider abstraction suitable for an in-process mobile model or remote runtime.

This should be attempted only after the mobile product demonstrates a need. It should use direct Swift/Kotlin bindings rather than hosting an unnecessary loopback HTTP server inside the app. The existing daemon should not be split into abstract crates merely in anticipation of this possibility.

## 16. Hosted runtime isolation

### 16.1 First implementation

The safest initial hosted vertical slice is one owner per isolated runtime deployment. Candidate isolation mechanisms include containers or microVMs, selected through measurement and threat modeling.

Each runtime requires:

- non-shared writable filesystem;
- owner-scoped database;
- owner-scoped process tree;
- owner-scoped provider and MCP credentials;
- restricted network egress;
- CPU, memory, disk and process limits;
- no access to host management sockets;
- structured logs with owner-safe metadata;
- lifecycle hooks for backup, upgrade and deletion.

### 16.2 Why a container alone is not the whole answer

Isolation also depends on:

- image supply chain;
- kernel and runtime hardening;
- egress control;
- secret injection;
- tool permissions;
- mounted filesystem scope;
- log redaction;
- control-plane authorization;
- backup access;
- host metadata exposure;
- incident response.

Arbitrary shell and command-based MCP functionality should be disabled in an early hosted beta unless the execution boundary is explicitly hardened for it.

### 16.3 Idle and wake behavior

A sleeping runtime can reduce cost, but introduces:

- first-interaction latency;
- queued proactive work;
- endpoint heartbeat behavior;
- voice-call timeout expectations;
- Copilot process/session restoration;
- update scheduling;
- race conditions when several surfaces wake it.

The product must measure acceptable wake latency rather than hiding it behind optimistic UI.

## 17. Authority, migration and split-brain prevention

### 17.1 Single-writer requirement

Two runtime incarnations must never both believe they are authoritative. This applies during:

- hosted rolling upgrades;
- regional migration;
- restore from backup;
- failover;
- transition between hosted and self-hosted operation;
- a Mac reconnecting after a remote runtime became authoritative.

### 17.2 Candidate fencing model

A control or coordination layer can issue a monotonic authority epoch. Every durable write and externally consequential turn occurs under the current epoch. A stale incarnation loses its lease and cannot renew authority.

Exact implementation is open, but acceptance tests should include:

- delayed network partitions;
- old process resurrection;
- duplicated wake requests;
- failed migration halfway through;
- restore from an old snapshot;
- credential rollback;
- message replay.

### 17.3 Hosted-to-self-hosted transfer

A credible transfer needs more than copying SQLite:

- export BoBe-owned data and files;
- identify external session state that cannot transfer directly;
- rotate credentials;
- establish a new runtime authority/epoch;
- revoke the prior runtime;
- update surfaces and endpoints;
- preserve an audit trail;
- verify the old hosted runtime can no longer act.

The reverse direction requires the same discipline.

## 18. Security and privacy requirements

### 18.1 Device security

Production device candidates should support:

- unique device identity;
- hardware-protected key use where practical;
- TLS server validation;
- Secure Boot;
- flash encryption;
- signed firmware;
- A/B update partitions;
- health-confirmed update commit;
- anti-rollback;
- credential rotation and revocation;
- explicit factory reset;
- no universal permanent fleet credential.

### 18.2 Surface authorization

A phone, Mac and toy do not need equal authority.

Example scope classes:

- conversation input/output;
- voice media;
- device state;
- memory read;
- memory administration;
- settings administration;
- device enrollment/revocation;
- provider/MCP administration;
- update administration.

A toy should normally receive only endpoint session, media and bounded device-control scopes.

### 18.3 Physical privacy

A consumer toy needs:

- hardware mute that cannot be overridden by software;
- visible recording/listening state;
- clear camera state where applicable;
- understandable retention behavior;
- no hidden offline audio accumulation;
- owner controls for proactive behavior;
- guest and child modes where the product serves households;
- deletion and export paths.

### 18.4 Child safety and consent

If marketed to or predictably used by children, architecture and product work must address jurisdiction-specific requirements before launch:

- parental consent;
- data minimization;
- retention and deletion;
- advertising and profiling restrictions;
- identity and age handling;
- inappropriate-content controls;
- emergency and dependency disclaimers;
- what adults can review;
- whether private child conversations exist and under what policy.

This cannot be added as a final moderation filter after hardware is manufactured.

### 18.5 End-of-service plan

A local-first physical product should define what happens if hosted BoBe ends:

- users can export their data;
- devices can be redirected to a self-hosted runtime;
- firmware verification does not depend forever on an unavailable service;
- essential provisioning documentation remains available;
- the device has a documented final supported firmware path;
- no remote kill switch unnecessarily bricks local operation.

## 19. OTA and release domains

Future BoBe will have independent update domains:

| Domain | Artifact | Trust and rollback concern |
|---|---|---|
| macOS | Signed/notarized app and bundled daemon | Existing Sparkle and Apple signing chain |
| Standalone runtime | Linux binary/container and migrations | Image/signature verification, schema compatibility, rollback |
| iOS | App Store application and bundled assets | Apple review, entitlements, model asset lifecycle |
| Android | Play-distributed application/native libraries | Play policy, foreground-service declarations, ABI support |
| ESP32 | Signed firmware and partitions | Secure Boot, A/B OTA, anti-rollback, recovery |
| Control plane | Service deployments and schema | Zero/low-downtime migration, API compatibility, tenant isolation |

One release must not require every domain to update atomically. Protocol versions and compatibility windows are necessary.

## 20. Data lifecycle and backup

### 20.1 Backup scope

A complete personal runtime backup may include:

- SQLite database;
- goals and memory files;
- configuration;
- worker session identifiers;
- installed-skill and MCP configuration;
- encrypted secret material or a re-provisioning manifest;
- runtime/version metadata;
- authority epoch metadata;
- endpoint registrations;
- optional caches only when redownload is impractical.

### 20.2 Restore requirements

Restore should prove:

- data integrity;
- schema compatibility;
- secret accessibility or explicit reauthentication;
- no concurrent old authority;
- endpoint reauthorization;
- Copilot session recovery or graceful fresh-session behavior;
- proactive schedules are not duplicated;
- deletion requests remain deleted.

A backup that has never been restored in a test is not a validated recovery system.

### 20.3 Hosted deletion

Deleting an owner must cover:

- active runtime;
- database and volumes;
- backups according to retention policy;
- logs and derived indexes;
- secret namespaces;
- device and surface credentials;
- control-plane metadata not legally required to remain;
- external provider data within BoBe's contractual control.

## 21. Repository and product boundaries

### 21.1 Near-term repository strategy

Do not split the current repository pre-emptively. Start with real artifact boundaries:

```text
bobrust (current)
  BoBeService/                  personal runtime
  BoBeMacUI/                    macOS surface
  docs/                         architecture and specialist designs
  protocol/                     only when a second implementation consumes it

bobe-device (when firmware starts)
  ESP-IDF firmware
  boards
  provisioning
  secure boot/OTA
  protocol fixtures
  hardware tests

bobe-control-plane (when hosted slice starts)
  account and device registry
  runtime directory/lifecycle
  routing
  fleet metadata
  hosted operations

bobe-android (when Android starts)
  native client and optional relay
```

An Apple repository or workspace split can be considered when iOS work begins. Shared Swift networking and DTO modules are useful; forcing macOS and iOS into one UI target is not.

### 21.2 When to extract Rust crates

Potential future crates:

- `bobe-domain` or `bobe-core` for portable domain behavior;
- `bobe-wire` for generated/versioned contracts;
- `bobe-runtime` for the personal authority;
- `bobe-daemon` for Axum/platform adapters.

Extraction should occur only when a real second consumer or platform boundary demonstrates the seam. The ESP32 does not need to link BoBe's Rust core; it needs a protocol.

### 21.3 One product family

Separate repositories do not imply separate products or identities. Product naming can remain:

- BoBe for the companion;
- BoBe for Mac/iPhone/Android as surfaces;
- BoBe Runtime for self-hosted operation;
- BoBe physical embodiments or room devices;
- BoBe Hosted as an operating option.

The user should choose where BoBe runs, not which incompatible BoBe they are buying.

## 22. Market and reference architectures

### 22.1 Home Assistant Voice Preview Edition

Useful evidence for:

- ESP32-S3 as a polished voice satellite;
- local wake word and physical privacy controls;
- local or cloud speech pipelines;
- open firmware and recoverability;
- separating endpoint hardware from server intelligence.

References: [Voice PE](https://www.home-assistant.io/voice-pe/), [firmware](https://github.com/esphome/home-assistant-voice-pe), [ESPHome voice assistant](https://esphome.io/components/voice_assistant/).

### 22.2 Xiaozhi

Useful evidence for:

- broad ESP32 board support;
- Opus and WebSocket/MQTT voice patterns;
- local wake word;
- server-side ASR, model and TTS;
- toy-like displays and actuators;
- self-deployable server implementations.

References: [firmware](https://github.com/78/xiaozhi-esp32), [server](https://github.com/xinnan-tech/xiaozhi-esp32-server).

It is a reference, not a production foundation to adopt unchanged. Its maintainers warn about completeness and security evaluation.

### 22.3 FoloToy and Willow

Useful evidence for toy/satellite products and self-hosted voice pipelines:

- [FoloToy](https://github.com/FoloToy/folotoy-server-self-hosting)
- [Willow](https://github.com/HeyWillow/willow)

They also illustrate that “self-hosted orchestration” may still use cloud speech or model providers. Locality must be stated per pipeline stage.

### 22.4 ESP RainMaker and IoT platforms

Useful evidence for:

- device claiming;
- fleet identity;
- firmware lifecycle;
- public versus privately operated service models;
- control-plane separation.

Reference: [ESP RainMaker](https://docs.rainmaker.espressif.com/docs/product_overview/).

### 22.5 Phone-gateway products

Omi, Brilliant Labs Frame and MentraOS demonstrate thin BLE devices using a phone as the active gateway/runtime:

- [Omi](https://github.com/BasedHardware/omi)
- [Frame SDK](https://docs.brilliant.xyz/frame/frame-sdk/)
- [MentraOS](https://github.com/Mentra-Community/MentraOS)

This validates a portable BLE mode, but not an assumption that the phone remains an invisible always-on daemon.

## 23. Differentiation

The market does not need another ESP32 endpoint that forwards audio to a generic LLM. BoBe's potential differentiation is continuity and ownership:

1. **One continuous relationship across bodies.**  
   The toy, phone and Mac do not reset the companion.

2. **Goals and long-term memory rather than isolated chat logs.**  
   BoBe's concept of the user extends beyond the last conversation.

3. **Proactive behavior grounded in user-controlled policy.**  
   Proactivity comes from the personal runtime, not generic engagement optimization.

4. **Hosted convenience without defining the companion as a cloud account.**  
   Self-hosting and export remain architectural possibilities.

5. **Transparent execution location.**  
   Users can understand which speech, model and tool stages are local or remote.

6. **Physical privacy and per-device permissions.**  
   A room toy, wearable and desktop do not receive identical authority.

7. **Credible hardware longevity.**  
   Purchased embodiments should have an end-of-service and self-hosting story.

8. **Native quality on each surface.**  
   Mac voice can exploit Apple Silicon while ESP32 uses a constrained endpoint protocol and mobile uses system capabilities where useful.

The product promise should be “the same BoBe can accompany you in different forms,” not “every form runs the same binary.”

## 24. Phased evidence plan

Phases are capability gates, not calendar commitments.

### Phase 0: Reconcile principles and current documentation

- Accept or amend the one-owner/one-authority invariant.
- Mark current versus proposed behavior consistently.
- Keep one authoritative runtime while allowing Mac-hosted speech adapters during the prototype.
- Define which document owns platform, voice, security and operations decisions.
- Create architecture decision records only for decisions actually made.

**Exit evidence:** no canonical documents contradict authority, route affinity or deployment vocabulary.

### Phase 1: Prove the runtime away from the Mac

- Isolate Apple-specific capture and permission code.
- Compile and test the daemon on Ubuntu x86_64.
- Package a standalone runtime.
- Make Copilot authentication host-neutral and operationally supportable.
- Connect the Mac through existing remote mode.
- Validate REST, SSE and voice over authenticated TLS.
- Define data root, supervision, logs, upgrade and rollback.
- Perform a real backup and restore.

**Exit evidence:** a documented fresh Ubuntu host can run one personal runtime and a Mac can use it end to end.

### Phase 2: Make the owner-facing transport multi-surface

- Replace the one-consumer main event queue.
- Add surface identity and scoped authorization.
- Add ordered/resumable events or snapshot fallback.
- Add source, target, turn and request correlation.
- Separate desktop presence from generic connection presence.
- Replace the global voice connection flag with an explicit lease model.
- Add compatibility and reconnection tests.

**Exit evidence:** Mac plus a second test client can operate without stealing events, leaking private results or duplicating turns.

### Phase 3: Build and validate an ESP32 reference endpoint

Use the recovered WonderBoy WB-12 ESP32-S3R8 audio/display body.

Start with:

- push-to-talk;
- one microphone/speaker path;
- Wi-Fi;
- secure WebSocket audio;
- correlated Swift STT adapter initially and daemon-owned TTS;
- basic face/LED state;
- development-only enrollment.

Measure:

- speech capture quality;
- transcript latency;
- time to first audio;
- interruption behavior;
- packet loss and reconnection;
- RAM/PSRAM;
- heat and power;
- echo and feedback;
- runtime wake latency in hosted and self-hosted paths.

**Exit evidence:** button press produces one authoritative turn and targeted text/audio on the toy while Mac SSE and Mac voice remain independently usable.

**Current evidence (July 17, 2026):** the mock body/adapter and real Swift
FluidAudio/Opus adapter both complete this software path with mTLS pinning,
private token/tool delivery, targeted captions/audio, playback drain,
conversation resync, and privacy purge. The recovered WB-12 firmware builds but
has not been flashed or acoustically measured because the unit is not connected.

### Phase 4: Production-grade device lifecycle

- QR/BLE claim prototype;
- unique device credentials;
- Secure Boot and flash encryption;
- signed A/B OTA and anti-rollback;
- revocation, reset and ownership transfer;
- bounded offline behavior;
- endpoint scopes and runtime registration;
- device protocol conformance fixtures.

**Exit evidence:** a device can be manufactured, claimed, updated, revoked, reset and recovered without a universal fleet secret or manual token copying.

### Phase 5: Hosted single-owner vertical slice

- Minimal account and runtime registry.
- One isolated runtime per test owner.
- Private volumes and secrets.
- Edge selection/routing.
- Authority epochs and fencing.
- Runtime start/stop/upgrade.
- Owner A/B isolation tests.
- Backup, restore and deletion.
- Restricted tool/MCP policy.

**Exit evidence:** two owners cannot access one another's content, secrets, events, processes or backups, including under failure tests.

### Phase 6: Mobile surfaces

- iPhone login, claiming and BLE Wi-Fi setup.
- iPhone chat, voice, notifications, settings and device controls.
- Local system STT/TTS where supported.
- Bounded local intelligence experiment.
- Android equivalent.
- Android connected-device service only if the use case and policy justify it.

**Exit evidence:** mobile improves onboarding and portability without becoming a hidden availability dependency.

### Phase 7: Product and manufacturing decision

Only after the preceding evidence:

- choose desk toy, room satellite, wearable or another first form;
- set bill-of-materials and retail-price targets;
- choose microphone/speaker/processor design;
- define child/household positioning;
- complete privacy and safety review;
- model hosted inference and support costs;
- decide whether custom hardware is justified.

**Exit evidence:** measured experience, economics, compliance and support requirements justify manufacturing.

## 25. Risk register

| Risk | Why it matters | Mitigation direction | Evidence signal |
|---|---|---|---|
| Cross-owner data access | Companion data is unusually sensitive | Runtime isolation, scoped control plane, adversarial isolation tests | No access across DB, files, secrets, events, logs or backups |
| Split authority | Duplicate actions and divergent memory | Epoch fencing and single-writer tests | Stale runtime cannot write or act |
| Poor far-field audio | Product feels unusable regardless of model quality | Board prototype, DSP/AEC testing, acoustic design | Measured recognition and barge-in performance by distance/noise |
| Hosted latency | Voice interaction loses immediacy | Regional placement, streaming, runtime prewake, local speech stages | Time-to-first-audio targets under real networks |
| Runtime idle cost | Process-per-owner may be expensive | Sleep/wake, density measurement, tiering | Cost per active and inactive owner |
| Wake latency | Sleeping runtime makes toy appear broken | Presence-triggered prewake, clear states, warm pools if justified | P50/P95 cold and warm interaction latency |
| Firmware compromise | Microphone and credentials are exposed | Secure Boot, signed OTA, unique keys, rollback protection | Verified boot/update/recovery test suite |
| Bricked update | Physical support cost and loss of trust | A/B partitions and health-confirmed commit | Power-loss and bad-image recovery tests |
| Service shutdown | Purchased hardware becomes e-waste | Self-host protocol, export, final firmware and documentation | Device redirected to self-hosted runtime without vendor service |
| Mobile suspension | Portable toy becomes unpredictably unavailable | Direct Wi-Fi mode, explicit relay states, reconnect design | Locked/background/process-death tests |
| Child safety/privacy | Legal and ethical harm | Product-scope decision before launch, consent/minimization controls | Jurisdictional review and auditable flows |
| Provider dependence | Copilot/model changes can break runtime | Adapter boundaries, session recovery, export, fallback policy | Provider outage and reauthentication exercises |
| Tool abuse | Hosted shell/MCP can affect external systems | Isolation, scoped credentials, confirmations, egress policy | Tool authorization and sandbox escape tests |
| Protocol drift | Rust, Swift, mobile and firmware disagree | Versioned schemas/fixtures and conformance suite | Cross-implementation compatibility CI |
| Documentation drift | Product claims exceed evidence | Status labels, ADRs, machine-checkable durable contracts | Documentation review in release gates |

## 26. Explicit no-go choices

### 26.1 Authority and security no-gos

- No concurrent writable authoritative runtime epochs for one owner.
- No plaintext unauthenticated non-loopback daemon exposure.
- No provider, MCP or broad administrative credentials in ESP32 firmware.
- No independent toy agent with its own canonical memory.
- No control plane in the routine decision path for local/self-hosted interactions unless explicitly chosen.
- No universal permanent fleet credential shared by all devices.
- No hidden microphone state or software-only claim of physical mute.
- No hosted arbitrary shell/tool execution without an explicit per-owner isolation boundary.

### 26.2 Proposed design no-gos

- Do not give toys the current global daemon bearer token.
- Do not connect toys directly to the current unversioned administrative API.
- Do not add `owner_id` throughout the current single-owner daemon as the first hosting strategy.
- Do not use the current destructive SSE queue for multiple surfaces.
- Do not force Mac voice through the constrained-device audio path.
- Do not make the hosted control plane mandatory for an already enrolled local/self-hosted runtime.

### 26.3 Mobile no-gos

- No claim that iPhone can be a continuously available general daemon.
- No abuse of background audio, location, Network Extension or other entitlements as keepalive tricks.
- No invisible or guaranteed-permanent Android service promise.
- No claim that compiling Rust proves the current Copilot runtime can ship on mobile.
- No downloading executable mobile runtime code outside permitted store mechanisms.

### 26.4 Scope no-gos

- No custom hardware before reference-board measurements.
- No repository split merely for architectural aesthetics.
- No protocol technology chosen because a comparable project uses it without BoBe measurements.
- No “fully local” marketing unless every relevant stage is local or each remote stage is clearly disclosed.

## 27. Open decisions

These require prototypes, product choices or ADRs.

### Product

1. What is the first physical form: desk toy, room satellite, portable toy, wearable, child companion or something else?
2. Who is the first customer, and what problem does the physical form solve better than the Mac/phone?
3. Is the product intended for children or households, and under which jurisdictions?
4. What retail price, bill of materials, battery life, microphone distance and speaker loudness are acceptable?
5. How much degraded/offline capability is necessary for trust?

### Runtime and hosting

6. What isolation substrate is sufficient for hosted tools: container, microVM or another boundary?
7. How long may a hosted runtime take to wake?
8. Which state must be backed up, and which external session state can only be recreated?
9. How is authority fencing implemented across migration and restore?
10. What is the supported path for hosted-to-self-hosted transfer?
11. Does the runtime use direct provider credentials, a managed agent service, or several selectable modes?

### Identity and routing

12. What account authentication method is used?
13. What identifies a stable runtime authority versus one incarnation?
14. Are devices authenticated with certificates, signed challenges, scoped tokens or a combination?
15. Can self-hosted enrollment work entirely without BoBe-hosted infrastructure?
16. Is hosted routing a reverse proxy, a relay, a rendezvous service or several modes?
17. Is application-layer end-to-end encryption required above TLS?

### Protocol

18. What is the first versioned owner-facing contract?
19. What event history and replay window are necessary?
20. Does endpoint state use WebSocket only or MQTT plus WebSocket?
21. Which codec and framing work best on the chosen hardware?
22. How are capabilities negotiated across firmware versions?
23. How are offline/local interactions reconciled without forking memory?

### Mobile

24. Is iPhone initially only setup plus surface, or also a BLE wearable relay?
25. Which Apple system-model/speech features are reliable across the supported device range?
26. Is an Android foreground service worth its UX and policy burden?
27. Is there a real need for a mobile-safe Rust core, or is native client logic sufficient?
28. What local data may mobile cache, and how is it encrypted/revoked?

### Operations and longevity

29. Who signs device firmware, and how are signing keys rotated?
30. How long are hardware and hosted runtime versions supported?
31. What is the final firmware/end-of-service procedure?
32. What observability is useful without collecting sensitive content?
33. How are compromised devices, surfaces and runtime credentials revoked?

## 28. What evidence would change this recommendation?

The architecture should remain falsifiable. The current recommendation could change if evidence shows:

- Process-per-owner cannot achieve viable cost even with sleep/wake and reasonable density. This would justify exploring stronger pooled runtime designs with tenant-aware state and sandboxed tools.
- A supported in-process Copilot or equivalent agent runtime becomes available for iOS/Android and mobile lifecycle tests show acceptable foreground/opportunistic behavior. This could justify a portable runtime mode.
- A target ESP-class device can run a sufficiently capable local speech/dialogue stack within power, quality and security constraints. More intelligence could move to endpoints while preserving canonical authority elsewhere.
- A single WebSocket cannot meet measured reliability or fleet-state needs. MQTT or another durable channel could become justified.
- Consumer research shows self-hosting has negligible value and materially harms onboarding. It may become an advanced/end-of-service path rather than a primary setup choice, but protocol portability and data export would still protect hardware longevity.
- Users strongly prefer household-shared rather than individual identity. The owner model may need household principals, delegated roles and multiple profiles while retaining one fenced authority.
- Child-safety obligations make an open-ended companion toy inappropriate for the first hardware product. The physical scope should narrow before weakening safety boundaries.

## 29. Immediate recommended next moves

The platform and physical-body tracks now need different evidence. For platform
portability, the next experiment should prove the core decentralization premise:

1. Make the current runtime compile and operate on Ubuntu x86_64.
2. Package one personal runtime as a supported standalone deployment.
3. Connect the existing Mac through secure remote mode.
4. Validate authentication, voice, events, restart, upgrade, backup and restore.
5. Replace the remaining single-client event assumptions.

This sequence answers the most important question first: can BoBe live independently of the Mac while remaining the same personal companion?

The BodyLink software seam has already advanced independently through mock and
real Swift-adapter validation. Its next evidence is narrower: guarded flashing
of the recovered WB-12, followed by capture, latency, playback, power, thermal,
reconnection, and recovery measurements. That physical validation does not need
to wait for Ubuntu packaging, but wake word, camera, broad board support, and
fleet lifecycle should wait until the PTT profile passes those measurements.

## 30. Summary

The proposed future is not a multi-user rewrite of the current daemon. It is a **personal-runtime platform**:

- every owner receives one fenced authoritative BoBe runtime;
- that runtime can be Mac-local, self-hosted or BoBe-hosted;
- a shared control plane makes hosted onboarding and operations easy without becoming the companion's memory or agent;
- Mac, iPhone and Android are native surfaces with different lifecycle capabilities;
- ESP32 toys are secure, expressive endpoints rather than weaker independent agents;
- protocols, identity, events and update systems are separated according to trust and resource constraints;
- hosted convenience and self-hosted longevity use the same conceptual companion contract;
- implementation advances only through measured capability gates.

The architectural purpose is to let BoBe gain new bodies without losing the qualities that make BoBe one personal, continuous and user-owned companion.

## 31. References

### Repository documents

- [BoBe Implementation Architecture](architecture.md)
- [Physical BoBe](physical-bobe.md)
- [Rust Guidelines](RUST_GUIDELINES.md)
- [Updating OTA](UpdatingOTA.md)

### Device and IoT references

- [ESP-IDF security](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/security.html)
- [AWS IoT fleet provisioning](https://docs.aws.amazon.com/iot/latest/developerguide/provision-wo-cert.html)
- [ESP RainMaker](https://docs.rainmaker.espressif.com/docs/product_overview/)
- [Matter](https://github.com/project-chip/connectedhomeip)

### Comparable open systems

- [Home Assistant Voice PE](https://www.home-assistant.io/voice-pe/)
- [Xiaozhi ESP32](https://github.com/78/xiaozhi-esp32)
- [Xiaozhi server](https://github.com/xinnan-tech/xiaozhi-esp32-server)
- [FoloToy](https://github.com/FoloToy/folotoy-server-self-hosting)
- [Willow](https://github.com/HeyWillow/willow)
- [Omi](https://github.com/BasedHardware/omi)
- [Brilliant Labs Frame](https://docs.brilliant.xyz/frame/frame-sdk/)
- [MentraOS](https://github.com/Mentra-Community/MentraOS)

### Mobile platform references

- [Rust iOS targets](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
- [Rust Android targets](https://doc.rust-lang.org/rustc/platform-support/android.html)
- [Tokio platforms](https://docs.rs/tokio/latest/tokio/#platforms)
- [Apple application lifecycle](https://developer.apple.com/documentation/uikit/managing-your-app-s-life-cycle)
- [Apple BackgroundTasks](https://developer.apple.com/documentation/backgroundtasks)
- [Apple App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/)
- [Apple Foundation Models](https://developer.apple.com/documentation/foundationmodels)
- [Android foreground services](https://developer.android.com/develop/background-work/services/fgs)
- [Android process lifecycle](https://developer.android.com/guide/components/activities/process-lifecycle)
- [Google Play foreground-service requirements](https://support.google.com/googleplay/android-developer/answer/13392821)
- [GitHub Copilot SDK](https://github.com/github/copilot-sdk)

### Hosted isolation references

- [AWS SaaS tenant isolation](https://docs.aws.amazon.com/wellarchitected/latest/saas-lens/tenant-isolation.html)
- [AWS multitenant API authorization](https://docs.aws.amazon.com/prescriptive-guidance/latest/saas-multitenant-api-access-authorization/introduction.html)
- [ThingsBoard tenant model](https://thingsboard.io/docs/user-guide/ui/tenants/)
