import { useEffect, useState } from "react";

import { getProfile, setProfile } from "./rpc";

/**
 * A harness for exercising the JSON-RPC contract from a browser, not the product UI.
 *
 * Edits the Profile as raw JSON.
 */
export default function App() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState("Loading…");

  useEffect(() => {
    load();
  }, []);

  async function load() {
    try {
      setText(JSON.stringify(await getProfile(), null, 2));
      setStatus("Loaded.");
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  async function save() {
    try {
      await setProfile(JSON.parse(text));
      setStatus("Saved.");
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  return (
    <main>
      <h1>Hoskinator</h1>
      <p>Profile, as JSON-RPC returns it.</p>
      <textarea
        rows={24}
        cols={72}
        value={text}
        spellCheck={false}
        aria-label="Profile as JSON"
        onChange={(event) => setText(event.target.value)}
      />
      <p>
        <button onClick={load}>Reload</button>{" "}
        <button onClick={save}>Save</button> <span>{status}</span>
      </p>
    </main>
  );
}
