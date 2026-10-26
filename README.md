# AlarmAnt

An emergency alert that cannot be ignored. A small sender raises a high-priority push; an Android app answers by playing an alarm on the alarm volume stream at maximum, showing a full-screen alert, and repeating until a person taps Acknowledge. If nobody acknowledges in time it escalates to a second person. A daily self-test proves the whole chain still works.

Status: pre-release, nothing built. This repository holds the charter, roadmap and patent record; see `ROADMAP.md`. The crates.io name `alarmant` is reserved at 0.0.0.

Shared by many portfolio projects (CireSnave, 2026-10-26: "Many of our projects will likely need that"). The Android app needs battery optimisation switched off for that one app; the daily self-test tells us when it stops waking.

Licence: MIT OR Apache-2.0.
