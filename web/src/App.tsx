import { useEffect, useState } from "react";

import {
  ENTRY_TYPES,
  createSection,
  deleteSection,
  getProfile,
  listSections,
  renameSection,
  retypeSection,
  setProfile,
  type Section,
} from "./rpc";

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
      <Sections />
    </main>
  );
}

/**
 * Exercises the four section methods. One button per JSON-RPC call, no client-side logic.
 */
function Sections() {
  const [sections, setSections] = useState<Section[]>([]);
  const [status, setStatus] = useState("Loading…");

  const [createName, setCreateName] = useState("");
  const [createType, setCreateType] = useState<string>(ENTRY_TYPES[0]);
  const [renameName, setRenameName] = useState("");
  const [renameTo, setRenameTo] = useState("");
  const [retypeName, setRetypeName] = useState("");
  const [retypeType, setRetypeType] = useState<string>(ENTRY_TYPES[0]);
  const [deleteName, setDeleteName] = useState("");

  useEffect(() => {
    load();
  }, []);

  async function load() {
    try {
      setSections(await listSections());
      setStatus("Loaded.");
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  /** Runs one call, then reloads the list. */
  async function run(call: () => Promise<unknown>, done: string) {
    try {
      await call();
      setStatus(done);
      setSections(await listSections());
    } catch (error) {
      setStatus(`Failed: ${(error as Error).message}`);
    }
  }

  return (
    <section>
      <h2>Sections</h2>
      <p>Sections, as JSON-RPC returns them.</p>
      <pre aria-label="Sections as JSON">{JSON.stringify(sections, null, 2)}</pre>
      <p>
        <input
          value={createName}
          aria-label="Name to create"
          onChange={(event) => setCreateName(event.target.value)}
        />{" "}
        <select
          value={createType}
          aria-label="Entry type to create with"
          onChange={(event) => setCreateType(event.target.value)}
        >
          {ENTRY_TYPES.map((entryType) => (
            <option key={entryType} value={entryType}>
              {entryType}
            </option>
          ))}
        </select>{" "}
        <button onClick={() => run(() => createSection(createName, createType), "Created.")}>
          Create
        </button>
      </p>
      <p>
        <input
          value={renameName}
          aria-label="Name to rename"
          onChange={(event) => setRenameName(event.target.value)}
        />{" "}
        <input
          value={renameTo}
          aria-label="New name"
          onChange={(event) => setRenameTo(event.target.value)}
        />{" "}
        <button onClick={() => run(() => renameSection(renameName, renameTo), "Renamed.")}>
          Rename
        </button>
      </p>
      <p>
        <input
          value={retypeName}
          aria-label="Name to retype"
          onChange={(event) => setRetypeName(event.target.value)}
        />{" "}
        <select
          value={retypeType}
          aria-label="Entry type to retype to"
          onChange={(event) => setRetypeType(event.target.value)}
        >
          {ENTRY_TYPES.map((entryType) => (
            <option key={entryType} value={entryType}>
              {entryType}
            </option>
          ))}
        </select>{" "}
        <button onClick={() => run(() => retypeSection(retypeName, retypeType), "Retyped.")}>
          Retype
        </button>
      </p>
      <p>
        <input
          value={deleteName}
          aria-label="Name to delete"
          onChange={(event) => setDeleteName(event.target.value)}
        />{" "}
        <button onClick={() => run(() => deleteSection(deleteName), "Deleted.")}>Delete</button>
      </p>
      <p>
        <button onClick={load}>Reload</button> <span>{status}</span>
      </p>
    </section>
  );
}
