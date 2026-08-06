import { useEffect, useState } from "react";

import {
  ENTRY_FIELDS,
  TEXT_FIELD,
  buildFields,
  isListField,
  readFields,
} from "./entryFields";
import {
  ENTRY_TYPES,
  callRepository,
  createEntry,
  createJobDescription,
  createSection,
  deleteEntry,
  deleteJobDescription,
  deleteSection,
  eligibleEntries,
  getEntry,
  getJobDescription,
  getProfile,
  listEntries,
  listJobDescriptions,
  listSections,
  renameSection,
  retypeSection,
  RpcFailure,
  setProfile,
  updateEntry,
  type Entry,
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
    }
  }

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

      <Entries />

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


/**
 * Exercises the six entry methods. One input per field the chosen type holds, generated from
 * ENTRY_FIELDS, and one button per JSON-RPC call.
 */
function Entries() {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [status, setStatus] = useState("Loading…");
  const [selected, setSelected] = useState<Entry | null>(null);

  const [entryType, setEntryType] = useState<string>(ENTRY_TYPES[0]);
  const [values, setValues] = useState<Record<string, string>>({});
  const [id, setId] = useState("");
  const [section, setSection] = useState("");
  const [listType, setListType] = useState<string>("");

  const fields = buildFields(entryType, values);

  useEffect(() => {
    void load();
  }, []);

  function describe(error: unknown) {
    return error instanceof RpcFailure
      ? `Failed (${error.code}): ${error.message}`
      : `Failed: ${(error as Error).message}`;
  }

  async function load() {
    try {
      setEntries(await listEntries(listType || null));
      setStatus("Loaded.");
    } catch (error) {
      setStatus(describe(error));
    }
  }

  /** Runs one call, keeps what it answered, then reloads the list. */
  async function run(call: () => Promise<Entry | null>, done: string) {
    try {
      setSelected(await call());
      setStatus(done);
      setEntries(await listEntries(listType || null));
    } catch (error) {
      setStatus(describe(error));
    }
  }

  /** Loads one entry into the form. */
  async function edit() {
    try {
      const entry = await getEntry(Number(id));
      setSelected(entry);
      if (entry) {
        setEntryType(entry.entry_type);
        setValues(readFields(entry.entry_type, entry.fields));
        setStatus("Loaded entry into the form.");
      } else {
        setStatus("No entry with that id.");
      }
    } catch (error) {
      setStatus(describe(error));
    }
  }

  function write(name: string, written: string) {
    setValues({ ...values, [name]: written });
  }

  /** Switching type clears the form. */
  function retype(chosen: string) {
    setEntryType(chosen);
    setValues({});
  }

  return (
    <section>
      <h2>Entries</h2>
      <p>Entries, as JSON-RPC returns them.</p>
      <pre aria-label="Entries as JSON">{JSON.stringify(entries, null, 2)}</pre>

      <p>
        <label>
          Entry type
          <select
            value={entryType}
            aria-label="Entry type"
            onChange={(event) => retype(event.target.value)}
          >
            {ENTRY_TYPES.map((type) => (
              <option key={type} value={type}>
                {type}
              </option>
            ))}
          </select>
        </label>
      </p>

      {entryType === "text" ? (
        <p>
          <label>
            {TEXT_FIELD}
            <br />
            <textarea
              rows={4}
              cols={72}
              value={values[TEXT_FIELD] ?? ""}
              spellCheck={false}
              aria-label={TEXT_FIELD}
              onChange={(event) => write(TEXT_FIELD, event.target.value)}
            />
          </label>
        </p>
      ) : (
        ENTRY_FIELDS[entryType].map((name) => (
          <p key={name}>
            <label>
              {name}
              {isListField(name) ? " (one per line)" : ""}
              <br />
              {isListField(name) ? (
                <textarea
                  rows={4}
                  cols={72}
                  value={values[name] ?? ""}
                  spellCheck={false}
                  aria-label={name}
                  onChange={(event) => write(name, event.target.value)}
                />
              ) : (
                <input
                  size={60}
                  value={values[name] ?? ""}
                  aria-label={name}
                  onChange={(event) => write(name, event.target.value)}
                />
              )}
            </label>
          </p>
        ))
      )}

      <p>Fields, as they will go over the wire.</p>
      <pre aria-label="Fields as JSON">{JSON.stringify(fields, null, 2)}</pre>

      <p>
        <button onClick={() => run(() => createEntry(entryType, fields), "Created.")}>
          Create
        </button>
      </p>

      <p>
        <label>
          Entry id
          <input value={id} aria-label="Entry id" onChange={(event) => setId(event.target.value)} />
        </label>{" "}
        <button onClick={edit}>Get</button>{" "}
        <button onClick={() => run(() => updateEntry(Number(id), fields), "Updated.")}>
          Update
        </button>{" "}
        <button
          onClick={() =>
            run(async () => {
              await deleteEntry(Number(id));
              return null;
            }, "Deleted.")
          }
        >
          Delete
        </button>
      </p>

      <p>
        <select
          value={listType}
          aria-label="Entry type to list"
          onChange={(event) => setListType(event.target.value)}
        >
          <option value="">every type</option>
          {ENTRY_TYPES.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>{" "}
        <button onClick={load}>List</button>
      </p>

      <p>
        <label>
          Section
          <input
            value={section}
            aria-label="Section to list eligible entries for"
            onChange={(event) => setSection(event.target.value)}
          />
        </label>{" "}
        <button
          onClick={async () => {
            try {
              setEntries(await eligibleEntries(section));
              setStatus("Listed what the section is eligible for.");
            } catch (error) {
              setStatus(describe(error));
            }
          }}
        >
          Eligible
        </button>{" "}
        <span>{status}</span>
      </p>

      <h3>Selected entry</h3>
      <pre aria-label="Selected entry">
        {selected ? JSON.stringify(selected, null, 2) : "No entry selected."}
      </pre>
    </section>
  );
}
