# Hugging Face access and credential onboarding

Status: Proposed

**Owner:** This file owns Optic Rift's Hugging Face token-entry and credential handling contract. The [model discovery contract](model-discovery-and-execution.md) owns model compatibility, intended use, licensing and download/runtime authorization. The [single architectural index](../ARCHITECTURE_INDEX.md) routes both. Do not duplicate one contract in another.

## Intended user journey

1. A creator opens **Models → Hugging Face access**. Public catalog browsing will remain possible without login once discovery ships.
2. **Get a Hugging Face read token** opens the browser to the same token-generation route used by Sonic Rift: https://huggingface.co/settings/tokens/new?tokenType=read. Hugging Face sign-in and token creation stay entirely outside Optic Rift.
3. Creator pastes a token in a masked form, explicitly saves it to **Windows Credential Manager**, then uses **Test connection**. A successful test is a live HTTPS call to Hugging Face's /api/whoami-v2; the token does not pass into browser local storage or project metadata.
4. Creator can disconnect and delete that credential at any time. A failed/missing vault or absent Tauri bridge must produce an error, never quietly save plaintext.
5. A connected account is **not** evidence that an individual gated model's terms were accepted, that an adapter supports the model, that installation happened, or that remote paid inference is enabled.

## Security boundary

- Windows native shell controls the credential store and the fixed token-page URL; frontend may request save/status/test/remove but may not retrieve token text through any command.
- Use the keyring crate's Windows-native backend. Do not reproduce Sonic Rift's plaintext \`app_settings.json\` token persistence or process-global \`HF_TOKEN\` propagation.
- Never log, echo, include in error strings, HTTP URL query or persist raw secrets to reports; outbound test uses only Authorization: Bearer over HTTPS to the fixed \`huggingface.co\` endpoint, no redirects.
- The app checks *shape*, not token permissions, before saving. Live whoami tests account validity only. No broad write scope is requested; recommend a fine-grained read-only token where appropriate.
- Password input clears after successful save; state queries yield only \`configured\`. Local tests must not use a real token. Gated repository terms require their own explicit user action.
- User media and model discovery must remain usable without the credential; a token enables eligible private/gated access later, but no automatic download or cloud upload.

## Review and acceptance

- The Rust/Cargo changes add dependencies; \`Cargo.lock\` must be updated and reviewed **on the actual local Windows host**. Do not advertise a successful Windows build until tested.
- Native tests: missing keyring, credential store error, no-entry, save/reopen/disconnect, malformed token, dropped network, revoked/invalid token, redirects, no leakage to logs and no secret returned over IPC.
- UI: token link opens in OS browser, password remains masked, status survives restart, disabled in browser-only preview, unhandled exceptions never dump tokens, account test request and offline states distinct.
- Verify the exact added commands against the Tauri capability boundary and the current Qortara governance checks, independently review native secret-handling and any manifest changes.
- Sonic Rift UX inspiration: https://github.com/Knapp-Kevin/sonic-rift/blob/main/this_voice_thing/ui/pages/model_settings.py. **Behavioral parity only**, not a copy of its token storage implementation.

**Observed status (2026-11-05, Windows 11 26300, branch `feature/optic-rift-brand-hf-token-20261105-no-ci` at `fa256c4`):** the first draft (`a1ad0aa`) did not compile (`hf_open_token_page` was not generic over the runtime), had no updated `Cargo.lock`, and failed `cargo fmt --check`; all three are fixed. Executed: real Windows Credential Manager save/trim/status/read/remove in an isolated test slot (the user's slot is never touched by tests); IPC status payload is `{"configured": bool}` only; local HTTP fixtures for 200, 401, 403, 302 (not followed), 429, 503, a non-responding server (timeout) and a refused port, none of whose messages contain the token; and a live `whoami-v2` call returning 401 for a bogus token. **Not yet executed:** a real user token (contract forbids test use), the browser-opening click in the running app, and independent review of native secret handling.
