```json
{
  "status": "FAIL",
  "reviewed_branch": "plan/phase-c",
  "reviewed_commit": "0beef1a8e213b9b55af85d55ca014e85fde52782",
  "errors": [],
  "scope": {
    "phase": "c",
    "sprint": null
  },
  "baselines_read": [
    "docs/requirements.md",
    "docs/architecture.md",
    "docs/project-plan.md",
    ".claude/project/quality-policy.md",
    "docs/bd-cache-store/requirements.md",
    "docs/bd-cache-sync/requirements.md",
    "docs/bd-cache-cli/requirements.md",
    "docs/bd-cache-py/requirements.md",
    "docs/sc-cron/requirements.md",
    "docs/bd-cache-api/requirements.md",
    "docs/sc-bd-cli/requirements.md"
  ],
  "phase_or_sprint_docs_read": [
    "docs/plans/phase-c/phase-c-plan.md",
    "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
    "docs/plans/phase-c/sprint-c-2-framework-application.md",
    "docs/plans/phase-c/sprint-c-3-template-upkeep.md",
    "docs/plans/phase-c/sprint-c-4-release-qualification.md"
  ],
  "requirement_checks": [
    {
      "id": "REQ-BDC-1001",
      "source": "docs/requirements.md:928",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-1002",
      "source": "docs/requirements.md:942",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ]
    },
    {
      "id": "REQ-BDC-1003",
      "source": "docs/requirements.md:957",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md",
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ]
    },
    {
      "id": "REQ-BDC-1005",
      "source": "docs/requirements.md:971",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ]
    },
    {
      "id": "REQ-BDC-0001",
      "source": "docs/requirements.md:63",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0006",
      "source": "docs/requirements.md:110",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md",
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ]
    },
    {
      "id": "REQ-BDC-0007",
      "source": "docs/requirements.md:118",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0008",
      "source": "docs/requirements.md:127",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0103",
      "source": "docs/requirements.md:156",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0502",
      "source": "docs/requirements.md:365",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0720",
      "source": "docs/requirements.md:493",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0721",
      "source": "docs/requirements.md:501",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ]
    },
    {
      "id": "REQ-BDC-0722",
      "source": "docs/requirements.md:509",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0723",
      "source": "docs/requirements.md:517",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0724",
      "source": "docs/requirements.md:525",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ]
    },
    {
      "id": "REQ-BDC-0725",
      "source": "docs/requirements.md:533",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0727",
      "source": "docs/requirements.md:549",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0730",
      "source": "docs/requirements.md:582",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0732",
      "source": "docs/requirements.md:598",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "REQ-BDC-0812",
      "source": "docs/requirements.md:719",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md",
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ]
    },
    {
      "id": "REQ-BDC-0813",
      "source": "docs/requirements.md:729",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ]
    },
    {
      "id": "REQ-BDC-0910",
      "source": "docs/requirements.md:803",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md",
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ]
    },
    {
      "id": "NFR-BDC-0001",
      "source": "docs/requirements.md:845",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "NFR-BDC-0002",
      "source": "docs/requirements.md:853",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "NFR-BDC-0003",
      "source": "docs/requirements.md:861",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/phase-c-plan.md"
      ]
    },
    {
      "id": "NFR-BDC-0004",
      "source": "docs/requirements.md:869",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "NFR-BDC-0005",
      "source": "docs/requirements.md:877",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md",
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ]
    },
    {
      "id": "NFR-BDC-0006",
      "source": "docs/requirements.md:885",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ]
    },
    {
      "id": "NFR-BDC-0007",
      "source": "docs/requirements.md:895",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ]
    },
    {
      "id": "REQ-CLI-0001",
      "source": "docs/bd-cache-cli/requirements.md:14",
      "listed_in_sprint_doc": true,
      "result": "violated",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:122"
      ],
      "notes": "Proposed dependency change needs an explicitly owned requirement amendment before implementation; current plan lacks it."
    }
  ],
  "deliverable_checks": [
    {
      "item": "c-1 deliverable 1: REQ-BDC-1002: land the already-tested rusqlite 0.39 alignment; move sc-command",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:121"
      ],
      "notes": "Planning commitment only; required dependency amendment/ownership missing (REQ-QA-002)."
    },
    {
      "item": "c-1 deliverable 2: REQ-BDC-1002, REQ-STORE-0003: implement upstream ScheduleStore with existing",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:126"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-1 deliverable 3: REQ-BDC-0720, 0724, 0725, 0730: replace TargetResolver/spawn_worker_with/run_due_with",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:131"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-1 deliverable 4: REQ-BDC-1002, REQ-API-0008: implement ADR-BDC-0016's narrow contract/logging",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:135"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-1 acceptance: lock/metadata show upstream sc-cron, rusqlite 0.39 and one SQLite link provider; existing CLI and wheel still build",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:190"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: open a pre-change v3 cache with schedules and pending journal entries, run migrated sync, reopen it; state and identity survive",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:191"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: real composition switches built-in \u2192 Python \u2192 built-in, including failed-switch rollback and same-process health",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:192"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: CLI and Python use migrated worker; timeout, second stop, hook child-tree kill and lease release obey current bounds",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:193"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: current outage and cached-read suites pass; cache hits invoke no bd and retain latency budget",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:194"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: contract-only sc-command path is permitted, clap/server/runtime leakage remains rejected in isolated consumer builds",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:195"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 acceptance: a configured label-filter scope and an --all scope both sync through the migrated worker as bd filters",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:196"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-1 required validation plan",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:208"
      ],
      "notes": "Required checks and integration proof are explicitly assigned; this is plan completeness, not implementation closeout."
    },
    {
      "item": "c-2 deliverable 1: REQ-BDC-1001: render the immutable template with the phase-plan answers and",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:112"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-2 deliverable 2: REQ-BDC-1001: preserve existing cached-read and Python execution without a",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:122"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-2 deliverable 3: REQ-BDC-1003: consume c-3's pin/answers/baseline helper, record the actual",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:128"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-2 deliverable 4: REQ-BDC-0812, REQ-BDC-0910: preserve existing CI and its OS/Python gates. Extend the current",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:135"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-2 deliverable 5: REQ-BDC-1001: execute and document one actual consumer workflow on the designated",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:140"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-2 acceptance: generated daemon runs a real scheduled sync against FakeBd; a pre-existing cache retains file/version/rows and schedule state",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:205"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 acceptance: /health stays live during backend outage; /ready reports degraded cached health without bd, then recovers from recorded state",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:206"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 acceptance: duplicate instance is refused; stop during a blocked hook is bounded, reaps children and then releases ownership; restart succeeds",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:207"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 acceptance: original bdc JSON/0-2-3 contract, alias identity, latency and Python corpus pass with daemon absent and present",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:208"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 acceptance: recorded pin/answers regenerate a baseline; a changed upstream template path appears in the diff without overwriting working files",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:209"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 acceptance: actual consumer testbed completes the supported workflow with recorded outcomes and bounded timing",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:210"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-2 required validation plan",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:212"
      ],
      "notes": "Required checks and integration proof are explicitly assigned; this is plan completeness, not implementation closeout."
    },
    {
      "item": "c-3 deliverable 1: REQ-BDC-1003: commit the immutable source tag/full SHA and generation-tool",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:46"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-3 deliverable 2: REQ-BDC-1003: implement a small regenerate-and-diff helper around the documented",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:52"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-3 deliverable 3: REQ-BDC-1003: document how c-2 records its adopted-path mapping and uses the",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:63"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-3 acceptance: recorded pin/answers regenerate a pristine baseline with registry dependencies",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:106"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-3 acceptance: an old/new fixture comparison obeys the mapping contract above for modified, added, deleted and unadopted paths and rejects invalid/stale mappings",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:107"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-3 acceptance: existing destinations are refused and the original worktree remains byte-identical on generator failure",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:108"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-3 acceptance: the pinned upstream generation path runs in an absent scratch destination; tool errors retain upstream diagnostics",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:109"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-3 required validation plan",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:111"
      ],
      "notes": "Required checks and integration proof are explicitly assigned; this is plan completeness, not implementation closeout."
    },
    {
      "item": "c-4 deliverable 1: REQ-BDC-1005, REQ-BDC-0813: update the consumer release input for published",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:63"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-4 deliverable 2: REQ-BDC-1005: use the kit's existing consumer preflight extension for manifest,",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:68"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-4 deliverable 3: REQ-BDC-1005: execute complete preflight and build/install candidate artifacts",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:72"
      ],
      "notes": "Concrete bounded planning commitment with requirement trace, owner and validation; future implementation not asserted present."
    },
    {
      "item": "c-4 acceptance: each supported target installs its declared binary/wheel set from the same commit/version; original CLI and new daemon smokes pass",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:84"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-4 acceptance: checksums cover exactly intended assets; no release configuration is uploaded; local sc-cron is absent from publish manifest",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:85"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-4 acceptance: stable preflight refuses an unfixed #135 kit and reports missing channel setup; supported prerelease path stays usable",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:86"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-4 acceptance: procedure rerun from installed artifacts produces the consumer outcomes c-2 established",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:87"
      ],
      "notes": "Positive observable and planted negative specified in authoritative sprint table."
    },
    {
      "item": "c-4 required validation plan",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:96"
      ],
      "notes": "Required checks and integration proof are explicitly assigned; this is plan completeness, not implementation closeout."
    }
  ],
  "findings": [
    {
      "id": "REQ-QA-001",
      "severity": "Blocking",
      "category": "requirement-not-listed",
      "source_refs": [
        ".claude/agents/req-qa.md:111",
        ".claude/project/quality-policy.md:13",
        "docs/bd-cache-store/requirements.md",
        "docs/bd-cache-sync/requirements.md",
        "docs/bd-cache-cli/requirements.md",
        "docs/bd-cache-py/requirements.md",
        "docs/sc-cron/requirements.md"
      ],
      "target_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:19"
      ],
      "issue": "c-1 owns every path in the store, sync, CLI, Python and local scheduler crates, but its authoritative requirements list omits governing approved crate requirements. Missing IDs by crate: {\"bd-cache-cli\": [\"REQ-CLI-0002\", \"REQ-CLI-0004\", \"REQ-CLI-0005\", \"REQ-CLI-0006\", \"REQ-CLI-0008\", \"REQ-CLI-0009\", \"REQ-CLI-0010\", \"REQ-CLI-0011\", \"REQ-CLI-0012\"], \"bd-cache-py\": [\"REQ-PY-0001\", \"REQ-PY-0003\", \"REQ-PY-0004\", \"REQ-PY-0005\", \"REQ-PY-0007\", \"REQ-PY-0008\", \"REQ-PY-0010\", \"REQ-PY-0011\", \"REQ-PY-0012\", \"REQ-PY-0013\", \"REQ-PY-0014\"], \"bd-cache-store\": [\"REQ-STORE-0001\", \"REQ-STORE-0004\", \"REQ-STORE-0005\", \"REQ-STORE-0006\", \"REQ-STORE-0007\", \"REQ-STORE-0008\"], \"bd-cache-sync\": [\"REQ-SYNC-0001\", \"REQ-SYNC-0004\", \"REQ-SYNC-0005\", \"REQ-SYNC-0010\", \"REQ-SYNC-0012\", \"REQ-SYNC-0013\"], \"sc-cron\": [\"REQ-CRON-0001\", \"REQ-CRON-0002\", \"REQ-CRON-0003\", \"REQ-CRON-0004\", \"REQ-CRON-0005\", \"REQ-CRON-0006\", \"REQ-CRON-0007\", \"REQ-CRON-0008\", \"REQ-CRON-0009\", \"REQ-CRON-0010\", \"REQ-CRON-0011\", \"REQ-CRON-0012\", \"REQ-CRON-0013\", \"REQ-CRON-0016\", \"REQ-CRON-0017\", \"REQ-CRON-0018\", \"REQ-CRON-0014\", \"REQ-CRON-0015\"]}. Directly touched repo constraints NFR-BDC-0001 (store/cache latency), NFR-BDC-0003 (incremental sync time), REQ-BDC-0703 (tick budget), REQ-BDC-0726 (consumer ingest) and REQ-BDC-0728 (failure-state marking) are also absent. This is a coverage defect even where preservation prose or existing tests already addresses the behavior.",
      "required_correction": "Complete c-1 requirements.constrained_by for the existing owned-path fence using the enumerated approved IDs and repo constraints. For prototype-only sc-cron IDs, state their disposition under the proposed upstream ownership transition rather than silently dropping guarantees or copying an obsolete API. Keep behavior unchanged; do not add a new checklist or gate.",
      "compliance_result": "partially-compliant"
    },
    {
      "id": "REQ-QA-002",
      "severity": "Blocking",
      "category": "cross-doc-conflict",
      "source_refs": [
        "docs/bd-cache-cli/requirements.md:14",
        "docs/requirements.md:942"
      ],
      "target_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:122",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:75"
      ],
      "issue": "c-1 explicitly moves sc-command to published 0.1.1, but its listed and binding REQ-CLI-0001 explicitly mandates a git dependency at v0.1.0. Proposed ADR-BDC-0016 names ADR-CLI-0001 and REQ-API-0008, but does not name REQ-CLI-0001; c-1 owns CLI architecture, not docs/bd-cache-cli/requirements.md. The plan therefore lacks an owned requirement amendment for the dependency-source/version change.",
      "required_correction": "Add the CLI requirements file to c-1 owned_paths and explicitly plan a narrowly approved amendment of REQ-CLI-0001 from the old git tag to the published baseline, preserving its version/envelope/usage and three-OS guarantees. Name that amendment in the existing proposed migration decision/requirement; do not silently change the Approved text or claim approval already exists.",
      "compliance_result": "non-compliant"
    },
    {
      "id": "REQ-QA-003",
      "severity": "Blocking",
      "category": "requirement-not-listed",
      "source_refs": [
        "docs/requirements.md:895"
      ],
      "target_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:20",
        "docs/plans/phase-c/sprint-c-2-framework-application.md:173"
      ],
      "issue": "c-2 defines a shared cache/schedule composition specifically to preserve one connection per process and forbids a second open, but omits governing NFR-BDC-0007 from its requirements list.",
      "required_correction": "Add NFR-BDC-0007 to c-2 requirements.constrained_by; retain the existing shared-composition and duplicate-open tests. No new behavior or artifact is needed.",
      "compliance_result": "partially-compliant"
    },
    {
      "id": "REQ-QA-004",
      "severity": "Blocking",
      "category": "requirement-not-listed",
      "source_refs": [
        "docs/bd-cache-py/requirements.md:178",
        "docs/bd-cache-py/requirements.md:262",
        "docs/requirements.md:877",
        "docs/requirements.md:885"
      ],
      "target_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:16",
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:91"
      ],
      "issue": "c-4 owns packaging/release qualification and explicitly promises the abi3-py39 platform/Python matrix and same commit/version across installed artifacts, but omits REQ-PY-0010 (wheel platforms and ABI) and REQ-PY-0014 (workspace/wheel version lockstep). Its new packaging unit tests and installed consumer procedure also touch NFR-BDC-0005 and NFR-BDC-0006, which are missing from the list. The narrow inherited Jev scope exclusions do not waive these repository-level requirements.",
      "required_correction": "List REQ-PY-0010, REQ-PY-0014, NFR-BDC-0005 and NFR-BDC-0006 as c-4 constraints. Keep live installed-consumer proof on the designated scratch testbed and helper unit tests deterministic/no-bd, as already intended; no additional matrix or process gate is needed.",
      "compliance_result": "partially-compliant"
    }
  ],
  "summary": {
    "total_findings": 4,
    "blocking_findings": 4,
    "important_findings": 0,
    "minor_findings": 0,
    "overall_compliance": "non-compliant",
    "deliverables_total": 15,
    "deliverables_complete": 14,
    "deliverables_incomplete": 1,
    "deliverable_completion_percent": 93.33,
    "counting_basis": "15 numbered sprint deliverables; 21 acceptance rows and 4 required-validation sections also checked separately below. Metadata omissions are additional gate findings, not extra product deliverables."
  },
  "gate_reason": "Plan QA1 FAIL: four blocking requirement coverage/baseline-amendment findings. No future implementation absence is scored as a defect. Scope and critical passes do not waive req-qa metadata coverage. Review used inherited Codex model, not unavailable Sonnet."
}
```
