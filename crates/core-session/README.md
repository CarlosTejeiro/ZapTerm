# core-session

Session lifecycle, state machine, and shared data types for ZapTerm.

## Purpose

Defines what a "session" is from the application's perspective: a saved connection configuration
(`Session` struct, `Protocol` enum) plus runtime lifecycle state. Drives the state transitions
connecting → connected → disconnected without knowing the transport details.

## Public API

_(Populated in Bloque 3)_

## Non-goals

- Does not own the transport layer (that is `core-transport`).
- Does not persist data to disk (that is `core-persistence`).
