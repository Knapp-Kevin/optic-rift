import { useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

type TokenState = { configured: boolean };
const trimmedError = (_: unknown) => "Hugging Face access request failed. See your local app diagnostics.";
export default function HuggingFaceAccess() {
  const [token, setToken] = useState("");
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState(false);

  useEffect(() => {
    let active = true;
    void invoke<TokenState>("hf_token_status")
      .then((s) => { if (active) setConfigured(s.configured); })
      .catch(() => { if (active) setConfigured(null); });
    return () => { active = false; };
  }, []);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    setError(false);
    try { await action(); }
    catch (e) {
      // Never put potentially sensitive backend/network error content into the UI.
      setMessage(trimmedError(e));
      setError(true);
    } finally { setBusy(false); }
  }

  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!/^hf_[a-zA-Z0-9_]{5,509}$/.test(token.trim())) {
      setMessage("Paste a Hugging Face access token that starts with hf_.");
      setError(true);
      return;
    }
    void run(async () => {
      const s = await invoke<TokenState>("hf_token_save", { token: token.trim() });
      setConfigured(s.configured);
      setToken("");
      setMessage("Token saved in Windows Credential Manager. Select Test connection to verify it.");
    });
  }

  return <section className="hf-access" aria-labelledby="hf-heading">
    <div className="hf-head">
      <div><h2 id="hf-heading">Hugging Face access</h2>
        <p>Optional for public models. Connect a read token to access your approved gated or private models when discovery and downloads become available.</p></div>
      <span className="hf-state">{configured === null ? "Native app required" : configured ? "Token stored" : "Not connected"}</span>
    </div>
    <div className="hf-steps">
      <span><b>1</b> Create a free read token</span><span><b>2</b> Paste and save it</span><span><b>3</b> Test your connection</span>
    </div>
    <button className="secondary hf-link" disabled={busy} onClick={() => void run(async () => {
      await invoke("hf_open_token_page");
      setMessage("Hugging Face should open in your browser. Create a read token and return here.");
    })}>Get a Hugging Face read token ↗</button>
    <form onSubmit={save} className="hf-form">
      <label htmlFor="hf-token">Access token (never shown again after saving)</label>
      <div className="hf-input-row">
        <input type="password" id="hf-token" value={token} onChange={e => setToken(e.target.value)}
          placeholder="hf_…" autoComplete="off" spellCheck={false} disabled={busy || configured === null} />
        <button type="submit" className="primary" disabled={busy || configured === null || !token.trim()}>Save token</button>
      </div>
    </form>
    <div className="hf-controls">
      <button className="secondary" disabled={busy || !configured} onClick={() => void run(async () => {
        const result = await invoke<string>("hf_token_test");
        setMessage(result);
      })}>Test connection</button>
      <button className="secondary" disabled={busy || !configured} onClick={() => void run(async () => {
        const s = await invoke<TokenState>("hf_token_remove");
        setConfigured(s.configured);
        setToken("");
        setMessage("Stored Hugging Face token removed.");
      })}>Disconnect</button>
    </div>
    {message && <p role={error ? "alert" : "status"} className={error ? "hf-error" : "hf-result"}>{message}</p>}
    <p className="hf-note">Stored only in your Windows credential vault. Optic Rift never saves tokens in projects, browser storage, or repository files. Testing contacts huggingface.co. Connecting does not download models or accept gated model terms.</p>
  </section>;
}
