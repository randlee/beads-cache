```json
{
  "schema_version": "1.0.0",
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "status": "prepared",
    "operation": "development_review",
    "candidate_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
    "item_file": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/item.txt",
    "account_file": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/account.md",
    "overrides_file": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/overrides.json",
    "judge_argv_file": "/tmp/phase-c-execution/c-1/qa-15/anchors/judge-argv.json",
    "preparation_manifest": "/tmp/phase-c-execution/c-1/qa-15/anchors/preparation.json",
    "preflight": {
      "success": true,
      "output_path": "/tmp/phase-c-execution/c-1/qa-15/anchors/preflight.json",
      "canonical_item": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/item.txt",
      "canonical_evidence": [
        "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/item.txt",
        "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/developer-result.md",
        "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/developer-result.json",
        "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/current-ci.json"
      ]
    },
    "dryrun": {
      "run_id": "dev-20261106T235524Z-c-1",
      "run_path": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/runs/dev-20261106T235524Z-c-1.json",
      "threshold": 0.6,
      "request_cap_bytes": 64000,
      "item_chars": 58243,
      "item_bytes": 58265,
      "results": 17,
      "requests": 16,
      "excluded": [
        {
          "anchor_id": "ADR-BDC-0009",
          "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
        }
      ],
      "shortened": [],
      "errors": [],
      "all_requests_contain_exact_complete_item": true,
      "all_requests_contain_sections": [
        "Sprint report",
        "Files changed",
        "Tests"
      ],
      "max_request_bytes_actual": 63847,
      "per_anchor": [
        {
          "anchor_id": "ADR-BDC-0005",
          "request_bytes": 63031,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ADR-STORE-0001",
          "request_bytes": 62999,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-001",
          "request_bytes": 63083,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-002",
          "request_bytes": 62948,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-003",
          "request_bytes": 62956,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-004",
          "request_bytes": 63847,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-008",
          "request_bytes": 62956,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "ANC-009",
          "request_bytes": 62933,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "NFR-BDC-0004",
          "request_bytes": 63636,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "NFR-BDC-0005",
          "request_bytes": 62962,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0001",
          "request_bytes": 62981,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0006",
          "request_bytes": 62966,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0007",
          "request_bytes": 63029,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0008",
          "request_bytes": 62998,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0502",
          "request_bytes": 62981,
          "item_chars_sent": 58243,
          "item_shortened": false
        },
        {
          "anchor_id": "REQ-BDC-0812",
          "request_bytes": 63021,
          "item_chars_sent": 58243,
          "item_shortened": false
        }
      ]
    },
    "live_judge_called_by_preparer": false,
    "notes": [
      "Complete current sprint is exact item prefix; only bounded appended account follows it.",
      "All 17 anchors retained, 16 requests constructed; ADR-BDC-0009 exclusion remains unchanged.",
      "No source, sprint, anchors, question fixtures, scope, exclusion, settings, tracker or lock files changed.",
      "Staged evidence is in ignored target/qa-15-anchor-evidence so canonical preflight accepts it."
    ],
    "sha256": {
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/item.txt": "a1a3a184ae20b8bf8bf73c81475910af5401e15a570c470cbd53f4103669f8f0",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/account.md": "8fa4e9715ab0e6d25e33b3f6c827a984a4c2934992770d8a7d9cc156810ac578",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/diffstat.txt": "ea70d06ed264f0264583747a2924d0d107c6c0a45599bdc8e53eae9ca69903c4",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/overrides.json": "79d928e7fd29d25900487127ff90181b502bf481565f54cae2537593715291cf",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/developer-result.md": "a367fd7705e25e0f11249195703b96a8242127bb7d60291ab243545c23d3d0c8",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/developer-result.json": "3042dc505debfa71a7c5f7225dcaa2620c154524a375958db1f5280e5213a2d0",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/current-ci.json": "96ae03ec1c61aa277d0a580ac9e5f5d583a2bd368cfe7c73596661d1846a31d7",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/docs/plans/phase-c/anchors/anchors.json": "19f70fcfd8ed07bec7cb04d1cfc6fabddadd21d537ca528ec937c5de2452f702",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/docs/plans/phase-c/sprint-c-1-runtime-migration.md": "46d77068c1771c20ef272d62ef08b458059089ef80d8b3115bbf853750985d2b",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/.codex/skills/plan-anchoring/scripts/jev_judge.py": "ea77e30a9cc433845cef8bd29453062ee7910c170418d20ea15cf3b78dcb60b6",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/.codex/agents/anchor-sprint-review.md": "6c0d4d63e52a5c1c43ff884091719b754009a87c85bdc9e4d75afdf700522b08",
      "/tmp/phase-c-execution/c-1/qa-15/anchor-runtime.json": "40ea691ad96e693097a3125d387e28b7b6e6af401207e2e4d6448e4c3c5b401d",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/runs/dev-20261106T235524Z-c-1.json": "c314eafdd187817bfca82ec629918402c3f602751687319dc0c089f59b738f53",
      "/tmp/phase-c-execution/c-1/qa-15/anchors/dryrun.json": "768824105479d6602c5870d34efaae38ccd0f2dd941201559dfbfb5d3ef4b80a",
      "/tmp/phase-c-execution/c-1/qa-15/anchors/preflight.json": "43308e94a40c7a8a2ad93b32e0d6895bebd019357b4d810af35f8607870fb314"
    }
  },
  "error": null,
  "metadata": {
    "correlation_id": "c-1-qa15"
  }
}
```
