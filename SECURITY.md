# Security policy

Lutetium is experimental and is not recommended for running untrusted programs. The bytecode verifier and VM runtime checks reduce some malformed-bytecode risks but are not a security sandbox. Builtin file operations use the permissions of the current process; native programs execute as ordinary native executables.

## Reporting a vulnerability

Please do not publish secrets, exploit payloads for systems you do not own, or personal data in a public issue. This repository currently has no verified private security contact configured. Until a private reporting channel is published, report suspected issues through a GitHub Security Advisory for the repository if available, or open a minimal public issue that avoids sensitive details and asks maintainers for a private follow-up channel.

Do not send credentials to project maintainers or include tokens in logs.
