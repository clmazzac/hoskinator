// Fills a running daemon's Master Store with fixture material for UI work.
//
// The person is invented and the facts are nonsense. This is not product data:
// the store ships empty, and nothing here should ever be treated as a default.
//
//   node scripts/seed-sample-store.mjs [--port 8737]

const port = Number(process.argv[process.argv.indexOf("--port") + 1]) || 8737;
const endpoint = `http://127.0.0.1:${port}/rpc`;

let nextId = 1;

async function rpc(method, params) {
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });
  if (!response.ok) {
    throw new Error(`${method}: the daemon answered ${response.status}`);
  }
  const answer = await response.json();
  if (answer.error) {
    throw new Error(`${method}: ${answer.error.message} (${answer.error.code})`);
  }
  return answer.result;
}

const PROFILE = {
  name: "Barnaby Q. Fenwhistle",
  headline: "Distributed systems engineer",
  location: "Ashgrove, Vermont",
  email: "barnaby@fenwhistle.example",
  website: "https://fenwhistle.example",
  social_networks: [{ network: "GitHub", username: "bfenwhistle" }],
};

// Sections are `{name, entry_type}`. Two `normal` sections are deliberate: an
// Entry is eligible for every Section whose type matches, so both list the same
// projects. That is the eligibility model, not a seeding mistake.
const SECTIONS = [
  ["Experience", "experience"],
  ["Selected Projects", "normal"],
  ["Open Source", "normal"],
  ["Education", "education"],
  ["Publications", "publication"],
  ["Skills", "one-line"],
];

// Each entry is [type, fields, bullets]. A bullet is [text, note, ...variants],
// where a variant is [text, note]. The first wording becomes the default.
const ENTRIES = [
  [
    "experience",
    {
      company: "Helio Systems",
      position: "Staff Software Engineer",
      location: "Remote",
      start_date: "2022-06",
      end_date: "present",
    },
    [
      [
        "Cut p99 latency on the ingest path from 840ms to 90ms by replacing per-row locking with a batched writer.",
        "the number-forward wording",
        [
          "Made ingest 9x faster at p99 by batching writes that had been taking a lock per row.",
          "shorter, for one-page resumes",
        ],
        [
          "Rewrote the ingest write path, removing a per-row lock that dominated tail latency.",
          "leads with the work, not the metric",
        ],
      ],
      [
        "Led a zero-downtime migration of 40 services off one shared Postgres instance onto per-service databases.",
        null,
        [
          "Split a shared Postgres instance into 40 per-service databases without downtime.",
          "punchier",
        ],
      ],
      [
        "Wrote the incident review process the platform org still runs on; median time to mitigate fell from 47 to 12 minutes.",
        null,
      ],
      [
        "Mentored four engineers, two of whom now lead their own teams.",
        "soft-skills bullet, usually cut first",
      ],
    ],
  ],
  [
    "experience",
    {
      company: "Ravensmoor Analytics",
      position: "Backend Engineer",
      location: "Burlington, VT",
      start_date: "2019-08",
      end_date: "2022-05",
    },
    [
      [
        "Built the event pipeline that carried 2.1 billion records a day at a steady 40MB/s per shard.",
        null,
        ["Built a 2.1B-record-per-day event pipeline.", "compressed"],
      ],
      [
        "Replaced a nightly batch job with an incremental reducer, cutting the reporting delay from 18 hours to 4 minutes.",
        "the one recruiters react to",
      ],
      [
        "Introduced property-based tests to the query planner and found 11 latent correctness bugs in the first week.",
        null,
      ],
    ],
  ],
  [
    "experience",
    {
      company: "Quillfeather Labs",
      position: "Software Engineering Intern",
      location: "Ashgrove, VT",
      start_date: "2018-06",
      end_date: "2018-08",
    },
    [
      [
        "Prototyped a columnar cache that the team shipped the following quarter.",
        null,
      ],
    ],
  ],
  [
    "normal",
    {
      name: "pgshard",
      date: "2023",
      summary: "A transparent sharding proxy for Postgres, in Rust.",
    },
    [
      [
        "Routes queries across 16 shards with no application changes, holding 60k queries a second on commodity hardware.",
        null,
        [
          "Sharding proxy holding 60k qps across 16 shards, transparent to the application.",
          "for the projects section when space is tight",
        ],
      ],
      [
        "Implements two-phase commit for cross-shard writes, verified with a deterministic simulation harness.",
        null,
      ],
    ],
  ],
  [
    "normal",
    {
      name: "tinyvm",
      date: "2021",
      summary: "A register-based bytecode VM used as teaching material.",
    },
    [
      [
        "Compiles and runs a small ML dialect in under 900 lines, used by three university compilers courses.",
        null,
        ["A 900-line bytecode VM adopted by three compilers courses.", "terse"],
      ],
      ["Ships a step debugger that visualises the register file at each instruction.", null],
    ],
  ],
  [
    "education",
    {
      institution: "Ashgrove College",
      area: "Computer Science",
      degree: "BS",
      location: "Ashgrove, VT",
      start_date: "2015-09",
      end_date: "2019-05",
    },
    [
      ["Graduated with honours; thesis on lock-free concurrent data structures.", null],
      ["Teaching assistant for operating systems across four semesters.", null],
    ],
  ],
  [
    "publication",
    {
      title: "Batched Commit Protocols for Sharded Relational Stores",
      authors: ["B. Q. Fenwhistle", "M. Underhay"],
      journal: "Journal of Imaginary Systems Research",
      date: "2024-03",
      doi: "10.0000/jisr.2024.0031",
    },
    [],
  ],
  [
    "publication",
    {
      title: "On the Futility of Nightly Batch Jobs",
      authors: ["B. Q. Fenwhistle"],
      journal: "Proceedings of the Ashgrove Workshop on Data Systems",
      date: "2022-11",
    },
    [],
  ],
  [
    "one-line",
    { label: "Languages", details: "Rust, Go, Python, TypeScript, SQL" },
    [],
  ],
  [
    "one-line",
    { label: "Systems", details: "Postgres, Kafka, Kubernetes, Redis, SQLite" },
    [],
  ],
  [
    "one-line",
    { label: "Practices", details: "Distributed tracing, property-based testing, incident review" },
    [],
  ],
];

async function main() {
  await rpc("profile.set", [PROFILE]);
  console.log(`profile   ${PROFILE.name}`);

  const existing = new Set((await rpc("section.list", [])).map((s) => s.name));
  for (const [name, entryType] of SECTIONS) {
    if (existing.has(name)) {
      console.log(`section   ${name} (already there)`);
      continue;
    }
    await rpc("section.create", [name, entryType]);
    console.log(`section   ${name} [${entryType}]`);
  }

  let bulletCount = 0;
  let variantCount = 0;
  for (const [entryType, fields, bullets] of ENTRIES) {
    const entry = await rpc("entry.create", [entryType, fields]);
    const label = fields.company ?? fields.name ?? fields.institution ?? fields.title ?? fields.label;
    console.log(`entry     ${label} [${entryType}]`);

    for (const [text, note, ...variants] of bullets) {
      const bullet = await rpc("bullet.create", [entry.id, text, note]);
      bulletCount += 1;
      variantCount += 1;
      for (const [variantText, variantNote] of variants) {
        await rpc("variant.create", [bullet.id, variantText, variantNote]);
        variantCount += 1;
      }
    }
  }

  console.log(
    `\n${ENTRIES.length} entries, ${bulletCount} bullets, ${variantCount} variants.`,
  );
}

await main();
