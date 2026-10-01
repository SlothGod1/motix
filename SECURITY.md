# Security

Security is a top-priority requirement. This document is the threat model, the list of required controls, and
the security testing plan. It is revisited at the end of every phase.

## Reporting a vulnerability

Please **do not** open a public issue. Use GitHub's private vulnerability reporting ("Report a vulnerability" on
the Security tab) once the repository is public. We aim to acknowledge within 7 days. Coordinated disclosure;
credit given unless you prefer otherwise.

---

## 1. Assets

| Asset | Why it matters |
|-------|----------------|
| User projects and media | Creative work, possibly private or unreleased content |
| User identity private keys | Impersonation; access to all servers/projects the user belongs to |
| Server identity private key | Impersonating the server to every client |
| Server data (encrypted projects, role logs, audit log, backups) | Integrity and availability of a team's work; content confidentiality is protected by E2EE even if the server is compromised |
| Project keys (MLS group state, asset keys, recovery kits) | Whoever holds them can read a project |
| Update signing keys (TUF root/targets, Authenticode) | Code execution on every user's machine |
| The user's machine | The app runs with the user's privileges |

## 2. Adversaries

- **Malicious media author** — crafts a video/image/font to exploit decoders.
- **Malicious project author** — shares a crafted `.motix` or template.
- **Malicious collaborator** — a legitimately invited user trying to exceed their role, crash others, or exfiltrate.
- **Network attacker** — on-path (café Wi-Fi, ISP), can observe/modify/replay packets.
- **Internet scanner/attacker** — targets exposed MOTIX Servers (DoS, auth bypass, exploitation).
- **Compromised or curious server/relay operator** — including a stolen server disk or leaked backup. Treated as untrusted for confidentiality (ADR-022).
- **Removed collaborator** — tries to keep reading a project after removal.
- **Future quantum adversary** — records encrypted traffic/storage today to decrypt later ("harvest now, decrypt later").
- **Supply-chain attacker** — compromises a dependency, GitHub Action, CI runner, or maintainer account.
- **Update attacker** — serves old/malicious updates, freezes clients on vulnerable versions.
- **Local malware** — already on the user's machine (out of scope to fully defend; we limit key exposure).

## 3. Trust boundaries and controls

### 3.1 Media files → engine
- Parsing and decoding only in `motix-media-worker` processes.
  - Phase 1: separate process, crash isolation, resource limits (memory, CPU time via job objects/cgroups/rlimits), timeouts.
  - Hardening phase: Windows restricted token + job object (no child processes, UI restrictions) → AppContainer; Linux seccomp-bpf allowlist + Landlock (read-only access to the granted file) + `no_new_privs` + namespaces where available; macOS (later phase) Seatbelt sandbox profile.
- Workers receive file handles (or a single read grant), not general filesystem access.
- Every message from a worker is **untrusted**: sizes, dimensions, timestamps, and metadata are validated in the main process (e.g. reject 1,000,000 × 1,000,000 frames, negative durations, absurd channel counts).
- FFmpeg built with only needed demuxers/decoders/protocols enabled; **network protocols disabled** (no `http`, `rtmp`, etc. — prevents SSRF-like attacks through playlists like HLS/concat); `concat`/`hls` demuxers disabled or restricted to prevent reading arbitrary local files.
- Images/fonts: prefer memory-safe Rust decoders (`image`, `swash`, `ttf-parser`) where quality allows.
- Keep FFmpeg current; security releases trigger a patch release of MOTIX.

### 3.2 Project files → engine
- SQLite opened with `SQLITE_DBCONFIG_DEFENSIVE`, `trusted_schema=OFF`, extension loading disabled, `cell_size_check=ON`, memory limits; integrity check on unclean files.
- `motix-schema` validates every object: types, ranges, string lengths, collection sizes, reference integrity, nesting depth (no composition cycles → no infinite recursion), keyframe counts.
- **No executable content** in projects: no scripts, no expressions evaluated with a general-purpose language (future expressions use a sandboxed, bounded interpreter).
- Media paths are *hints*: resolved only to regular files; never followed into device files/UNC admin shares automatically; relative paths normalized (`..` segments cannot escape the allowed roots when packaged projects are extracted).
- Package/zip extraction: path traversal ("zip slip") protection, size and file-count limits, symlinks rejected.

### 3.3 Network → client / server
- Transport: iroh QUIC with TLS 1.3, raw public keys, **mutual authentication**, post-quantum hybrid key exchange. No plaintext protocol exists.
- **Authentication ≠ authorization.** Every connection-level operation (fetch, append, admin) is checked against the signed role log at the authority (host or server); every content-level operation (edit) is additionally checked by every receiving client (§3.4).
- **Invites**: 256-bit secrets, expiring, single-use by default, role-scoped, revocable; proof is bound to the TLS channel (no replay); invite secrets never logged.
- **Replay**: QUIC protects the transport; 0-RTT is disabled for state-changing operations; CRDT updates carry (peer, counter) ids and are idempotent; invite nonces are tracked.
- **Resource limits**: connection rate limits per key and per IP, handshake budgets, max message sizes enforced before allocation, per-connection memory budgets, bounded queues, idle and slow-peer timeouts, per-role operation budgets.
- **Update validation**: every receiving client decrypts, verifies signature and role, applies to a scratch fork and validates with `motix-schema` before accepting. Rejection is deterministic so all clients agree. The server, which cannot decrypt, enforces only sender role and size/rate limits.
- **Media transfer**: encrypted, BLAKE3-verified streaming of ciphertext; only to members with read access; per-user bandwidth/storage quotas on servers.
- **Server identity pinning**: key change at a known server → hard warning, no silent reconnect.
- **Privilege escalation**: role changes only by owners/admins, always audited; server admin actions require the admin role on a fresh authenticated connection; no "first user is admin" race (claim code required).

### 3.4 End-to-end encryption of collaboration (ADR-022)
- All project content in collaboration (document updates, snapshots, presence, media, proxies, thumbnails) is encrypted end to end with MLS-derived keys; relays and servers handle ciphertext only.
- Every update is signed by its author's device key and verified — along with the author's role in the signed role log — by **every receiving client** before it is applied. A compromised server cannot forge, alter or replay edits; it can only drop or delay them, which clients detect through per-author hash chains and version-vector gossip.
- Member removal triggers an MLS Commit and key rotation; removed devices cannot decrypt anything newer.
- Post-quantum: hybrid X25519MLKEM768 transport handshakes; hybrid X-Wing MLS ciphersuite for group key agreement (draft; migration path kept).
- The opt-in **Helper** role is the only way a server can read a project; it is per project, visible to all members, and revocable (with key rotation).
- Safety codes let members verify each other and the server out of band; key changes of verified parties block until the user confirms.
- Recovery kits: owners are prompted to save one; snapshot keys are sealed to the owner's recovery public key so a project survives device loss.
- Known limits (documented honestly to users): metadata (who, when, sizes, IPs) is visible to the server; a removed member keeps what they already downloaded; anyone with read access can copy what they see.

### 3.5 Server host
- Runs as an unprivileged service account (`motix-server` user on Linux, a virtual service account on Windows).
- The local admin web UI binds to `127.0.0.1` only, with a one-time token; remote administration goes through the authenticated `motix/admin/1` protocol.
- No shell-outs with user-controlled strings (no command injection surface); parameterized SQL only (no SQL injection surface); filesystem access confined to the data-dir.
- Data-at-rest: project content is already end-to-end encrypted; the server identity key is encrypted at rest; backups are ciphertext and safe off-site. OS volume encryption still recommended (protects metadata).

### 3.6 Local secrets and logging
- Identity keys in OS keystore where available; fallback file encrypted with a passphrase (Argon2id KDF + age).
- `Secret<T>` wrapper types never print their contents; logs never contain private keys, invite secrets, tokens, or project content; diagnostic reports are redacted and shown to the user before sharing.
- Memory holding keys is zeroized on drop (`zeroize`).

### 3.7 Updates and supply chain
- TUF (ADR-012): root key offline (owner-held; paper/USB backup; later hardware key and 2-of-3 threshold), targets key used only in the approved `release` environment, online timestamp/snapshot keys with short expiry.
- Authenticode signing of all Windows executables and DLLs we ship (OD-3).
- Launcher verifies before executing anything; staged files are written to user-owned directories with restrictive ACLs; no execution from temp directories.
- CI: GitHub Actions pinned by commit SHA; `permissions:` minimal per job; release jobs require manual approval; branch protection + required reviews; signed tags; 2FA required for maintainers.
- Dependencies: `Cargo.lock` committed; `cargo-deny` (licenses, banned crates, duplicate versions, advisories) and `cargo-audit` on every PR; new dependencies require a short justification in the PR; SBOM (CycloneDX) attached to each release; build provenance attestations.
- FFmpeg and other C/C++ dependencies built from pinned, hash-verified source archives.

**Update signing (preview channel, ADR-027).** Releases are accepted only if `SHA256SUMS` carries a valid Ed25519 signature from a key compiled into MOTIX and names the exact release version; the download must match its signed SHA-256. The secret key lives only in the `MOTIX_UPDATE_SIGNING_KEY` GitHub Actions secret (available to pushes on `main`, never to pull requests) plus one offline backup held by the owner. If the key leaks: remove the secret, generate a new pair (`cargo run -p xtask -- keygen`), ship a release trusting only the new key, and announce it. Zip extraction rejects absolute paths, `..`, and the updater's own folder; sizes and entry counts are bounded; files are swapped with rollback.

**Shared network folder (ADR-031).** Versions are verified exactly as above before MOTIX installs them into a shared folder, and a version folder is never modified after it's written. The launcher starts whatever `versions\current` names, so write access to the share must be limited to trusted people (like any shared program folder); per-user settings and projects are never stored there. Version names are parsed as version numbers (no path separators), archives are unpacked with the same path checks, and old versions are only deleted after a rename proves no PC is running them.

**Project files (ADR-032).** Opening a `.motix` file is untrusted input: 64 MB cap, item limits, unknown fields rejected, duplicate ids refused, names stripped of control characters and capped, times range-checked, clips that overlap or point at missing tracks dropped; saving is atomic with a `.bak`. Crash-recovery copies stay in the user's local app-data folder.

**Creator Lab owner password (ADR-033).** Only a random salt and SHA-256(Argon2id(password)) with 64 MiB / 3 passes are in the public code; the remembered unlock is the derived key in the user's own app-data folder. Minimum 12 characters. The password must never be typed into a chat or a GitHub issue. This gates the official build's UI; it is not a security boundary against someone who builds their own copy.

### 3.8 AI models
- Only ONNX/GGUF formats (no pickle / arbitrary code deserialization).
- Pinned hashes in TUF-signed catalog; downloaded over HTTPS and verified; inference in `motix-ai-worker` with the same sandboxing as media workers.

### 3.9 Plugins (future)
- WebAssembly components with explicit, user-visible capabilities; no ambient filesystem/network; resource limits (fuel/epochs, memory caps).
- Shader effects validated by naga; GPU timeouts handled (device-lost recovery).

---

## 4. Security testing plan

| Test | How | From phase |
|------|-----|-----------|
| Malformed media | cargo-fuzz on probe/IPC decoding; corpus of broken files; worker crash → app survives | 1 |
| Malicious projects | Fuzz `.motix` loader and schema validator; fixtures for cycles, huge values, path traversal | 1 |
| Zip slip / path traversal | Unit + property tests on package extraction and path resolution | 1 |
| Update attacks | TUF test vectors: expired metadata, rollback, wrong signatures, mix-and-match, oversized targets, tampered downloads, key rotation | 2 |
| Rollback/health check | Fault-injected builds that crash on start → launcher reverts | 2 |
| Authentication bypass | Integration tests: wrong key, unregistered key, revoked key, expired/used/forged invites | 4 |
| Authorization bypass | Viewer/commenter attempting edits, member attempting admin ops, cross-project access | 4 |
| Replay | Replayed handshakes/invite proofs, duplicated updates | 4 |
| Malformed packets | Fuzz every protocol message decoder; property tests on framing | 4 |
| Rate-limit bypass | Load tests: many keys from one IP, many IPs per key, slowloris-style peers | 5 |
| Privilege escalation | Role-change paths, claim-code race, admin-protocol misuse | 5 |
| SQL/command injection | Static checks: no string-built SQL (lint), no shell execution APIs in server crates (cargo-deny ban + clippy lint) | 5 |
| Dependency vulnerabilities | cargo-audit/deny daily scheduled run + on PR | 0 |
| Session hijacking | Connection migration/resumption tests; token-scoped admin UI tests | 5 |
| E2EE: server can't read | Inspect everything a server/relay stores or forwards — no plaintext content, no content keys (automated scan of stored bytes for known plaintext) | 4 |
| E2EE: forged/altered edits | Malicious server modifies, reorders, replays or injects updates → every client rejects them identically | 4 |
| E2EE: removed member | Removed device cannot decrypt updates or new media after removal | 4 |
| E2EE: withholding detection | Server drops one author's updates → clients raise a warning | 5 |
| E2EE: role enforcement | Viewer/Commenter signed edits rejected by all clients even if the server forwards them | 4 |
| Recovery kit | Lose all member devices → restore from recovery kit → project readable | 5 |
| Key-change warnings | Changed server/collaborator key → connection blocked until confirmed | 4 |
| Sandbox escape attempts | Worker tries to open files/network/processes → denied (tests per OS) | hardening |

Security review checklist is part of every phase's completion criteria ([PHASES.md](PHASES.md)).

---

## 5. Secure defaults summary

- Update checks: on, **notify only** for servers, automatic (with user choice) for clients.
- Invites: expire in 24 h, single use, role Editor (temporary sessions) / Viewer (servers) unless changed.
- Host approval for joins in temporary sessions: on.
- End-to-end encryption: always on for collaboration; server Helper: off.
- Post-quantum hybrid handshakes: on.
- Server admin web UI: localhost only.
- Firewall: never changed without consent.
- Telemetry: none.
