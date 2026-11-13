kind: added
---
Registration (f): operator commands. `alarmant enroll --contact first|second` makes the one-time enrolment code and shows it once; `alarmant revoke --contact first|second` deletes the device and voids its in-flight ack tokens. `--contact` is required and never guessed; there is no command that undoes a revocation. This slice is the command logic (`admin::parse`, `admin::run`); the binary and PersistAnt stores come with slice (g). Only `first` runs until the registry holds two recipients; `second` is refused with a plain message and changes nothing.
