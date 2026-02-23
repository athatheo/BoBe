# BoBe System Design

> Architecture specification for the proactive AI companion system

**Version:** 1.0
**Last Updated:** 2026-01-26

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Architecture](#2-architecture)
3. [Provider Architecture](#3-provider-architecture) ← **Core abstraction layer**
4. [Component Deep Dive](#4-component-deep-dive)
5. [API Specification](#5-api-specification)
6. [Data Models](#6-data-models)
7. [Event System](#7-event-system)
8. [Orchestration Engine](#8-orchestration-engine)
9. [Voice Pipeline](#9-voice-pipeline)
10. [Storage Design](#10-storage-design)
11. [Deployment & Configuration](#11-deployment--configuration)

---

# 1. System Overview

### 1.1 What is BoBe?

BoBe is a **local-first, proactive AI companion** for Mac and PC that:

- **Observes** your screen activity through periodic captures
- **Understands** context using OCR and vision-capable LLMs
- **Remembers** what you're working on in a private, on-device memory
- **Proactively reaches out** when it has something helpful to say
- **Listens and speaks** via local speech-to-text and text-to-speech

**The key insight**: Most AI assistants wait for you to ask. BoBe watches what you're doing and offers help when it notices you might need it—like a thoughtful colleague who says "Hey, I noticed you've been stuck on that error for a while, want me to take a look?"

### 1.2 Design Principles

| Principle                | Implementation                                        | Rationale                                |
| ------------------------ | ----------------------------------------------------- | ---------------------------------------- |
| **Local-first**          | All processing on-device by default                   | Privacy, speed, works offline            |
| **Privacy-preserving**   | Data never leaves machine unless user opts in         | Screen content is sensitive              |
| **Non-intrusive**        | Proactive but not annoying; learns when to engage     | Bad UX kills adoption                    |
| **Thin client UI**       | Electron shell is pure display; all logic in daemon   | Separation of concerns                   |
| **Graceful degradation** | Works without LLM, just captures; works without voice | Partial functionality > no functionality |

### 1.3 Design Philosophy

**Why proactive, not reactive?**

Traditional assistants require the user to context-switch: stop what you're doing, formulate a question, wait for response, apply the answer, return to work. This interrupts flow state.

BoBe inverts this: it watches your work context continuously and _decides_ when help would be valuable. The user never has to ask—they just receive timely, contextual assistance.

**The "thoughtful colleague" mental model:**

Imagine a knowledgeable colleague sitting next to you. They:

- Don't interrupt you every 5 seconds
- Notice when you're struggling (same error for 10 minutes)
- Offer help at natural breakpoints (when you pause)
- Remember what you were working on yesterday
- Know when to stay quiet (you're in flow)

This is what BoBe aims to be.

### 1.4 High-Level Data Flow

```txt
┌─────────────────────────────────────────────────────────────────────────────┐
│                              USER'S DESKTOP                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Screen Activity ──────┐                                                   │
│   Voice Input ──────────┤                                                   │
│   Clipboard ────────────┤                                                   │
│                         ▼                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                        bobe-daemon (Python)                         │   │
│   │                                                                     │   │
│   │   Capture ──▶ Extract ──▶ Contextualize ──▶ Decide ──▶ Generate     │   │
│   │                                               │                     │   │
│   │                                               ▼                     │   │
│   │                                         Should I speak?             │   │
│   │                                           │       │                 │   │
│   │                                          No      Yes                │   │
│   │                                           │       │                 │   │
│   │                                         idle    notify              │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                         │                                                   │
│                         │ SSE events + HTTP                                 │
│                         ▼                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      bobe-shell (Electron)                          │   │
│   │                      [separate repository]                          │   │
│   │                                                                     │   │
│   │   Overlay ◀── State Display ◀── Speech Bubble ◀── Voice Output      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Architecture

### 2.1 Process Model

BoBe runs as **three separate processes**:

| Process        | Role               | Technology        | Repository    |
| -------------- | ------------------ | ----------------- | ------------- |
| `bobe`         | All business logic | Python + Litestar | This repo     |
| `bobe-shell`   | Desktop overlay UI | Electron + React  | Separate repo |
| `llama-server` | LLM inference      | llama.cpp (C++)   | Third-party   |

### 2.2 Why Separate Processes?

**Isolation**: If the UI crashes, the daemon keeps running (context isn't lost). If the daemon crashes, the UI can show an error and attempt reconnection. Neither takes down the other.

**Resource management**: The LLM server can be memory-mapped separately. On memory pressure, the OS can prioritize differently. Users can run different LLM servers (llama.cpp, Ollama, vLLM) without touching the daemon.

**Development velocity**: UI team can iterate without touching backend. Backend can be tested headlessly. Different release cadences are possible.

**Flexibility**: The shell could be reimplemented in native code. The daemon could serve multiple UI clients. The LLM server can be swapped for cloud APIs.

---

## Backend API Reference

Get the full API schema from the backend:

```bash
cd /Users/john/Repos/ProactiveAI
/Users/john/Repos/ProactiveAI/scripts/dump_api_schema.sh
```

Key endpoints:

- `GET /health` - Health check
- `GET /status` - Current state
- `GET /events` - SSE stream
- `POST /message` - Send user message
- `POST /capture/start` - Start capture
- `POST /capture/stop` - Stop capture
