import { useEffect, useState } from "react";

import {
  ENTRY_TYPES,
  callRepository,
  createJobDescription,
  createSection,
  deleteJobDescription,
  deleteSection,
  getJobDescription,
  getProfile,
  listJobDescriptions,
  listSections,
  renameSection,
  retypeSection,
  RpcFailure,
  setProfile,
  type JobDescription,
  type Section,
} from "./rpc";

/** A harness for exercising the JSON-RPC contract from a browser, not the product UI. */
export default function App() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState("Loading…");
  const [title, setTitle] = useState("");
  const [jobText, setJobText] = useState("");
  const [query, setQuery] = useState("");
  const [jobDescriptions, setJobDescriptions] = useState<JobDescription[]>([]);
  const [selected, setSelected] = useState<JobDescription | null>(null);
  const [jobDescriptionStatus, setJobDescriptionStatus] = useState("Loading…");
  const [repositoryRequest, setRepositoryRequest] = useState(
    '{\n  "method": "repository.status",\n  "params": []\n}',
  );
  const [repositoryResult, setRepositoryResult] = useState("");
  const [repositoryStatus, setRepositoryStatus] = useState("");

  useEffect(() => {
    void load();
    void loadJobDescriptions();
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

  async function loadJobDescriptions() {
    try {
      const search = query.trim();
      setJobDescriptions(await listJobDescriptions(search || null));
      setJobDescriptionStatus("Loaded.");
    } catch (error) {
      setJobDescriptionStatus(`Failed: ${(error as Error).message}`);
    }
  }

  async function create() {
    try {
      const created = await createJobDescription({
        title: title.trim() || undefined,
        text: jobText,
      });
      setSelected(created);
      setTitle("");
      setJobText("");
      await loadJobDescriptions();
      setJobDescriptionStatus("Created.");
    } catch (error) {
      setJobDescriptionStatus(`Failed: ${(error as Error).message}`);
    }
  }

  async function inspect(id: number) {
    try {
      const jobDescription = await getJobDescription(id);
      setSelected(jobDescription);
      setJobDescriptionStatus(
        jobDescription ? "Loaded Job Description." : "Job Description no longer exists.",
      );
    } catch (error) {
      setJobDescriptionStatus(`Failed: ${(error as Error).message}`);
    }
  }

  async function remove(id: number) {
    try {
      const deleted = await deleteJobDescription(id);
      if (selected?.id === id) {
        setSelected(null);
      }
      await loadJobDescriptions();
      setJobDescriptionStatus(deleted ? "Deleted." : "Job Description no longer exists.");
    } catch (error) {
      setJobDescriptionStatus(`Failed: ${(error as Error).message}`);
  async function callRepositoryRequest() {
    try {
      const request = JSON.parse(repositoryRequest);
      const result = await callRepository(request);
      setRepositoryResult(JSON.stringify(result, null, 2));
      setRepositoryStatus("Succeeded.");
    } catch (error) {
      const detail = error instanceof RpcFailure
        ? `Failed (${error.code}): ${error.message}`
        : `Failed: ${(error as Error).message}`;
      setRepositoryStatus(detail);
    }
  }

  return (
    <main>
      <h1>Hoskinator</h1>
      <section>
        <h2>Profile</h2>
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
          <button onClick={load}>{"Reload"}</button>{" "}
          <button onClick={save}>{"Save"}</button> <span>{status}</span>
        </p>
      </section>

      <hr />

      <Sections />

      <hr />

      <section>
        <h2>Job Descriptions</h2>
        <p>
          <label>
            Title or label
            <input value={title} onChange={(event) => setTitle(event.target.value)} />
          </label>
        </p>
        <p>
          <label>
            Pasted posting text
            <br />
            <textarea
              rows={16}
              cols={72}
              value={jobText}
              spellCheck={false}
              aria-label="Pasted Job Description"
              onChange={(event) => setJobText(event.target.value)}
            />
          </label>
        </p>
        <p>
          <button onClick={create}>Create</button>
        </p>

        <p>
          <label>
            Full-text search
            <input value={query} onChange={(event) => setQuery(event.target.value)} />
          </label>{" "}
          <button onClick={loadJobDescriptions}>List</button> <span>{jobDescriptionStatus}</span>
        </p>
        <ul>
          {jobDescriptions.map((jobDescription) => (
            <li key={jobDescription.id}>
              <button onClick={() => inspect(jobDescription.id)}>
                {jobDescription.title || `(untitled #${jobDescription.id})`}
              </button>{" "}
              <button onClick={() => remove(jobDescription.id)}>Delete</button>
            </li>
          ))}
        </ul>

        <h3>Selected Job Description</h3>
        <pre aria-label="Selected Job Description">
          {selected ? JSON.stringify(selected, null, 2) : "Select a Job Description."}
        </pre>
      </section>
      <hr />
      <h2>Repository JSON-RPC</h2>
      <textarea
        rows={12}
        cols={72}
        value={repositoryRequest}
        spellCheck={false}
        aria-label="Repository JSON-RPC request"
        onChange={(event) => setRepositoryRequest(event.target.value)}
      />
      <p>
        <button onClick={callRepositoryRequest}>Call</button>{" "}
        <span>{repositoryStatus}</span>
      </p>
      <textarea
        rows={24}
        cols={72}
        value={repositoryResult}
        readOnly
        spellCheck={false}
        aria-label="Repository JSON-RPC response"
      />
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
