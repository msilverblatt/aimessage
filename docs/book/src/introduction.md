# Introduction

AiMessage turns any Mac into an iMessage API server. Single binary, zero external dependencies.

## How it works

- **Read path**: Polls `~/Library/Messages/chat.db` (SQLite) for new messages and reactions, tracking the highest processed ROWID so it resumes correctly after restart.
- **Write path**: Sends messages and attachments via `osascript` controlling Messages.app — no private frameworks required for basic sending.
- **Advanced features** (optional): Reactions and typing indicators via Apple's private IMCore framework, which requires SIP to be disabled.

## What you can build

AiMessage exposes iMessage as a standard REST API with webhook and WebSocket delivery. Some examples:

- **Chatbots and AI agents** — receive incoming messages via webhook or WebSocket, process them, and reply via the send endpoint.
- **CRM integrations** — pipe conversations into your customer data platform by forwarding webhook events.
- **Auto-responders** — trigger automated replies based on keywords, schedules, or external conditions.
- **Notification systems** — use iMessage as a delivery channel for alerts, monitoring events, or two-factor codes.

## Requirements

- macOS Ventura or later
- Rust toolchain (`rustup`, `cargo`)
- Messages.app signed into an Apple ID
