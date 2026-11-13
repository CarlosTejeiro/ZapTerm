# core-config

Application settings, themes, and terminal profiles for ZapTerm.

## Purpose

Defines the typed Rust representations for all user-configurable settings: global app settings,
terminal profiles (font, size, colour scheme), theme definitions, and per-session overrides.

## Public API

_(Populated in Bloque 7)_

## Non-goals

- Does not persist data to disk — calls `core-persistence` for that.
- Does not apply themes to the terminal renderer — that is the frontend.
