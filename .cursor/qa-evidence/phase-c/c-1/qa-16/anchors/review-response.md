```json
{
  "schema_version": "1.0.0",
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "dev",
    "item_id": "c-1",
    "run_id": "dev-20261107T002343Z-c-1",
    "run_path": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-16-anchor-evidence/runs/dev-20261107T002343Z-c-1.json",
    "dry_run": false,
    "reused_run": false,
    "pass": true,
    "failing": [],
    "excluded": [
      {
        "anchor_id": "ADR-BDC-0009",
        "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
      }
    ],
    "errors": [],
    "results": [
      {
        "anchor_id": "ADR-BDC-0005",
        "source_type": "ADR",
        "source_ref": "ADR-BDC-0005",
        "statement": "When bd cannot be reached, beads-cache must keep answering reads from the cache and record an outage for the affected scopes.",
        "quote": "When bd cannot be reached, beads-cache must keep answering reads from the cache and record an outage for the affected scopes.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "the outage tests show reads still answered and the scope's outage recorded on spawn failure, timeout and transport errors",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.95,
              "no": 0.03,
              "unclear": 0.02
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.28,
            "probabilities": {
              "yes": 0.52,
              "no": 0.44,
              "unclear": 0.04
            }
          }
        },
        "min_confidence": 0.92,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15389,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60812
      },
      {
        "anchor_id": "ADR-STORE-0001",
        "source_type": "ADR",
        "source_ref": "ADR-STORE-0001",
        "statement": "The beads-cache store file must keep the v3 tables and indexes unchanged, adding only nullable columns and new tables.",
        "quote": "The beads-cache store file must keep the v3 tables and indexes unchanged, adding only nullable columns and new tables.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 4,
        "deterministic_check": "the format golden test matches the exact v3 DDL; PRAGMA user_version stays 3",
        "kind": "prohibition",
        "leaning": "asserted_only",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.52,
            "probabilities": {
              "yes": 0.68,
              "no": 0.1,
              "unclear": 0.22
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "yes": 0.02,
              "no": 0.98,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.46,
            "probabilities": {
              "yes": 0.26,
              "no": 0.64,
              "unclear": 0.1
            }
          }
        },
        "min_confidence": 0.97,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15384,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60780
      },
      {
        "anchor_id": "ANC-001",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "Words on the prohibited list of the repository's no-consumer lint (`scripts/lint_no_atm.py`) must not appear in this repository outside that lint, its tests and `docs/team-protocol.md`; the lint passes on the whole tree.",
        "quote": "blocking [a prohibited word: the consumer's cron principal] and the other words on the lint's prohibited list from source or tests (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 4,
        "deterministic_check": "`python3 scripts/lint_no_atm.py` exits 0 on the whole tree; per sprint, `python3 scripts/lint_no_atm.py --paths <sprint fence>` exits 0",
        "kind": "structural",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.95,
              "no": 0.04,
              "unclear": 0.01
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.96,
            "probabilities": {
              "yes": 0.97,
              "no": 0.02,
              "unclear": 0.01
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.79,
            "probabilities": {
              "yes": 0.12,
              "no": 0.86,
              "unclear": 0.02
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          }
        },
        "min_confidence": 0.79,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15407,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60864
      },
      {
        "anchor_id": "ANC-002",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "beads-cache must run its sync of bd, the beads issue-tracker CLI, at the times set in a SQLite cron table.",
        "quote": "beads-cache needs to handle the synchronization of bd on a cron tick. [...] long term sync would be done by beads-cache at a time scheduled in the cron scheduler in sqlite cron table.",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 4,
        "deterministic_check": "a test runs the built-in sync from an sc-cron schedule row in the cache file",
        "kind": "directive",
        "leaning": "asserted_only",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.95,
              "no": 0.03,
              "unclear": 0.02
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.24,
            "probabilities": {
              "yes": 0.46,
              "no": 0.49,
              "unclear": 0.05
            }
          }
        },
        "min_confidence": 0.92,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15380,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60729
      },
      {
        "anchor_id": "ANC-003",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "beads-cache must let a consumer-registered Python sync method replace its hard-coded bd sync.",
        "quote": "In the event of the 'hard-coded sync' not working, the hard-coded sync could be disabled and replaced w/ a python method registered by consumer [the first consumer, whose name is a prohibited word]. (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 4,
        "deterministic_check": "an end-to-end test disables the built-in sync row and a registered Python sync ingests a scope",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.86,
            "probabilities": {
              "yes": 0.91,
              "no": 0.05,
              "unclear": 0.04
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.38,
            "probabilities": {
              "yes": 0.59,
              "no": 0.37,
              "unclear": 0.04
            }
          }
        },
        "min_confidence": 0.86,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15374,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60737
      },
      {
        "anchor_id": "ANC-004",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "beads-cache must support an optional consumer-registered post-sync hook that receives the bd sync results and errors.",
        "quote": "there can also be hooks necessary to report/escelate based on [the first consumer's] requirements [...] We would also likely want a 'post-sync' hook that could contain sync results and errors and allow consumer [the first consumer] to do custom things at that point (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 5,
        "deterministic_check": "an end-to-end test asserts the post-sync hook request carries each scope's result and error",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": true
        },
        "questions_sha256": "1b8029d3df4c3a0d8556760eec14561ad02bab0f2121fee3ab85aa94ee891d5e",
        "questions": {
          "relevant": {
            "criteria": {
              "no": "What was touched is something else: a command that only starts the existing sync, write-journal escalation, or an unrelated part of the system.",
              "unclear": "It cannot be told from the account what code or behaviour was touched.",
              "yes": "What was created, changed, or removed is the bd sync job's own behaviour, its result or failure reporting, or something that runs after a bd sync."
            },
            "instructions": "Identify the code or behaviour that the work in `item.text` created, changed, or removed. Is that the bd sync job's own behaviour (how it runs, what it reports, how its failures are escalated) or behaviour that runs after a bd sync? A command that merely starts the existing sync, and the write journal's own escalation, are not the sync job's behaviour. Judge only what was touched.",
            "type": "choice"
          },
          "implemented": {
            "criteria": {
              "no": "The account is silent about such a hook, mentions it only as a topic, plan or later work, or describes a hook that is removed, mandatory, closed to consumers, or not given both results and errors.",
              "unclear": "The account mentions a post-sync hook as delivered but it is not possible to tell whether it is given the sync results and errors.",
              "yes": "The account says the delivered work calls a hook or callback after the sync and passes it the sync results and errors, and does not make it mandatory or closed to consumers."
            },
            "instructions": "`anchor.statement` requires an optional post-sync hook that a consumer registers and that receives the bd sync results and errors. According to `item.text`, does the delivered work provide such a hook: something run after the sync that is given the sync's results and its errors? Count words such as post-sync hook, after-sync callback, or hook named in the row's payload. It does not count if the hook is only discussed, planned, or deferred, if it is removed, if errors or results are kept from it, if it is made mandatory, or if consumers cannot register one. Judge whether the account says this was delivered, not whether it proves it.",
            "type": "choice"
          },
          "violated": {
            "criteria": {
              "no": "The described design does not rule it out: it provides the hook, says nothing about it, or leaves it for later.",
              "unclear": "The description touches the hook or the sync results but it cannot be told whether the design rules the required hook out.",
              "yes": "The described design explicitly rules out the required hook in one of those ways."
            },
            "instructions": "Does the delivered design described in `item.text` rule out what `anchor.statement` requires: an optional, consumer-registered hook that runs after the bd sync and receives the results and errors? An explicit statement that no hook runs after the sync, that results and errors go only elsewhere, that errors or results are withheld from the hook, that a hook is required, or that consumers cannot register one, rules it out. Silence about the hook, or a plan to add it later, does not. Judge conflict only.",
            "type": "choice"
          },
          "evidenced": {
            "criteria": {
              "no": "`item.text` makes that claim with nothing checkable, or makes no such claim.",
              "unclear": "Something that might be evidence is cited, but it is not clear that it supports the claim.",
              "yes": "`item.text` claims the hook was delivered with results and errors and points to at least one checkable artifact (path, commit, PR, command, test) supporting it."
            },
            "instructions": "The compliance claim that matters is that the delivered work provides an optional, consumer-registered hook that runs after the bd sync and receives the sync results and errors. Does `item.text` make that claim and cite something checkable that supports it: a file path, commit, PR or diff reference, a command that was run, or a test name or test output? A bare assertion does not count. If `item.text` makes no such claim, for example it is silent, defers the hook, or describes a hook that is removed, required, closed to consumers, or not given both results and errors, answer no.",
            "type": "choice"
          }
        },
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.08,
              "unclear": 0.0,
              "yes": 0.92
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.03,
              "unclear": 0.01,
              "yes": 0.96
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          }
        },
        "min_confidence": 0.88,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15595,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 61628
      },
      {
        "anchor_id": "ANC-008",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "Each repository must have at most one built-in beads-cache bd sync schedule in sc-cron, the SQLite cron job scheduler.",
        "quote": "I expect 'sc-cron' to be a general tool that can be used for scheduling jobs.  limiting 'builtin' sync to one per repo makes sense.",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 5,
        "deterministic_check": "a test shows a second built-in sync row is refused while other rows are allowed",
        "kind": "prohibition",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": true
        },
        "questions_sha256": "0513e743fd340219e1a4fe960e27da347688718cc6dcba5dc12be56b8da1903b",
        "questions": {
          "relevant": {
            "type": "choice",
            "instructions": "The state holds a description of delivered work (`item.title`, `item.text`) and one anchor: a directive the user stated verbatim in `anchor.statement`, with the area it governs in `anchor.scope`. Does the delivered work described in `item.text` touch the subject that `anchor.statement` talks about? Judge overlap of subject matter only. Do not judge compliance.",
            "criteria": {
              "yes": "The work changed, created, removed, or depends on the thing the anchor statement is about.",
              "no": "The work is in a different part of the system or a different activity, and it could neither follow nor break the anchor.",
              "unclear": "The description is too vague to tell what the work touched."
            }
          },
          "implemented": {
            "criteria": {
              "no": "It says the limit was not kept, only plans it, or makes no claim that it was kept.",
              "unclear": "It mentions the built-in limit but leaves the delivered cardinality uncertain.",
              "yes": "It says the delivered scheduler limits each repository to zero or one built-in sync row or refuses a second such row."
            },
            "instructions": "Does `item.text` state that the delivered work kept at most one built-in beads-cache bd sync schedule row for each repository? Judge the claimed delivered restriction only; silence, future intent, and discussion of scheduling alone do not state that it was kept.",
            "type": "choice"
          },
          "violated": {
            "type": "choice",
            "instructions": "Does `item.text` show that the work did something `anchor.statement` rules out, or used a different approach than the one `anchor.statement` requires? Examples: the anchor says to start from an existing implementation and the work was written from scratch; the anchor forbids a dependency and the work added it; the anchor fixes a boundary and the work crossed it. Silence about the directive is not a violation. Judge conflict only.",
            "criteria": {
              "yes": "The description states an action, approach, or decision in the delivered work that cannot be reconciled with the directive.",
              "no": "Nothing in the description conflicts with the directive, whether or not the directive is mentioned.",
              "unclear": "The description is ambiguous enough that the work might or might not conflict with the directive."
            }
          },
          "evidenced": {
            "type": "choice",
            "instructions": "Where `item.text` claims the directive in `anchor.statement` was followed, does it cite concrete, checkable evidence for that claim: file paths, commit or diff references, commands that were run, test names or output, or a comparison against the required source? A bare assertion such as 'we followed the anchor' or 'based on X' with nothing checkable does not count. If the description does not claim the directive was followed at all, answer no.",
            "criteria": {
              "yes": "The description points to at least one checkable artifact (path, diff, command, test, comparison) that supports the compliance claim.",
              "no": "The compliance claim is asserted without anything checkable, or no compliance claim is made.",
              "unclear": "Something that might be evidence is mentioned, but it is not clear that it supports the compliance claim."
            }
          }
        },
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.64,
            "probabilities": {
              "no": 0.19,
              "unclear": 0.05,
              "yes": 0.76
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.77,
            "probabilities": {
              "yes": 0.12,
              "no": 0.85,
              "unclear": 0.03
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.7,
            "probabilities": {
              "yes": 0.8,
              "no": 0.18,
              "unclear": 0.02
            }
          }
        },
        "min_confidence": 0.77,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15333,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60511
      },
      {
        "anchor_id": "ANC-009",
        "source_type": "ANC",
        "source_ref": null,
        "statement": "beads-cache sync scopes must be defined as bd filters, such as labels or --all, instead of as phases.",
        "quote": "0702 Active phases - would NEED to be defined w/ filters.  'phase' is a concept of [the first consumer and its messaging tool, whose names are prohibited words].  other applications would have different scopes including --all. (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)",
        "stated_by": "Rand",
        "stated_at": "2026-10-25",
        "context": "phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement",
        "anchor_version": 5,
        "deterministic_check": "tests sync a label-filter scope and an --all scope; grep finds no phase-file reading in crates/",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.74,
            "probabilities": {
              "yes": 0.83,
              "no": 0.16,
              "unclear": 0.01
            }
          }
        },
        "min_confidence": 0.98,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15373,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60714
      },
      {
        "anchor_id": "NFR-BDC-0004",
        "source_type": "REQ",
        "source_ref": "NFR-BDC-0004",
        "statement": "Each bd call, Python sync method and post-sync hook started by beads-cache must be killed at its timeout, capped by the time left before the run's deadline.",
        "quote": "Each bd call, Python sync method and post-sync hook started by beads-cache must be killed at its timeout, capped by the time left before the run's deadline.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "a test with a hanging fake bd and a hanging hook shows each is killed at its timeout and the run ends by its deadline",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": true
        },
        "questions_sha256": "f5c60135ba5580c25b752234f611e141855a10ade483c2d7cd0cf63b0d6f77d3",
        "questions": {
          "relevant": {
            "criteria": {
              "no": "The work does not add, change, or run any bd call, Python sync method run, or post-sync hook run: for example CI lint, release packaging, documentation, or CLI output formatting that leaves those processes unchanged.",
              "unclear": "The description is too vague to tell whether the work touches a bd call, a Python sync method run, or a post-sync hook run.",
              "yes": "The work adds, changes, or runs a bd call, a Python sync method run, or a post-sync hook run, or changes how one of them is started, timed, or stopped."
            },
            "instructions": "`item.text` describes delivered work. `anchor.scope` lists the kinds of work this anchor governs: work that adds or changes a bd call, a Python sync method run, or a post-sync hook run started by beads-cache. Does the delivered work in `item.text` add, change, or run one of those three kinds of process, or change how one is started, waited on, timed, or stopped? Judge only whether the work is of that kind. Do not judge whether the work complied.",
            "type": "choice"
          },
          "implemented": {
            "criteria": {
              "no": "The account does not say this: it is silent about killing at a timeout, only names the requirement or the topic, defers it, describes a process it runs as having no timeout or as exempt, or kills at a timeout not limited by the time left.",
              "unclear": "The account describes a timeout or kill, but it cannot be told whether the process is killed or whether its timeout is limited by the time left.",
              "yes": "Each process the account describes is said to be killed (terminated, stopped) at its timeout limited by the time left: wording such as min(timeout, time left), the smaller of its limit and the remaining run time, or capped by the deadline."
            },
            "instructions": "`anchor.statement` requires that bd calls, Python sync method runs and post-sync hook runs started by beads-cache are killed at their timeout, with the timeout capped by the time left before the run's deadline. Does `item.text` say that the delivered work kills the processes it describes in this way? An account that covers only some of the three kinds counts when it says nothing about the others; what matters is that each process it describes as run is killed at a timeout limited by the time left. A requirement ID, a topic mention, or a plan for later does not count. Do not judge evidence or conflict.",
            "type": "choice"
          },
          "violated": {
            "criteria": {
              "no": "The text states no such thing, whether or not it mentions timeouts.",
              "unclear": "The text says something about timeouts that may mean a process is unbounded or uncapped, but it is ambiguous.",
              "yes": "The text explicitly states that one of those processes has no timeout, is exempt, is awaited however long it takes, or may run past the run's deadline."
            },
            "instructions": "Does `item.text` state that some bd call, Python sync method run, or post-sync hook run is not killed at its timeout, or is given a timeout that is not capped by the time left before the run's deadline, contrary to `anchor.statement`? Only an explicit statement of such behaviour counts. Not mentioning timeouts is not a conflict.",
            "type": "choice"
          },
          "evidenced": {
            "criteria": {
              "no": "There is no such claim, or the claim is made with nothing checkable.",
              "unclear": "Something checkable is mentioned, but it is not clear that it backs the claim.",
              "yes": "The claim is backed by at least one checkable artifact of the kinds listed."
            },
            "instructions": "If `item.text` claims that bd calls, Python sync method runs, or post-sync hook runs are killed at their timeout capped by the time left before the deadline, does it cite something checkable that backs that claim: a test whose name or description concerns timeouts, deadlines, hanging processes, or kills; a source file path for that behaviour; a commit or diff; a command with its output; or a CI run? A bare claim does not count. If `item.text` makes no such claim, answer no.",
            "type": "choice"
          }
        },
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.66,
            "probabilities": {
              "no": 0.2,
              "unclear": 0.03,
              "yes": 0.77
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.03,
              "unclear": 0.01,
              "yes": 0.96
            }
          }
        },
        "min_confidence": 0.66,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15580,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 61417
      },
      {
        "anchor_id": "NFR-BDC-0005",
        "source_type": "REQ",
        "source_ref": "NFR-BDC-0005",
        "statement": "Unit tests for beads-cache must run with no bd and no Dolt, using the deterministic fake bd.",
        "quote": "Unit tests for beads-cache must run with no bd and no Dolt, using the deterministic fake bd.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "CI unit jobs run with no bd on PATH; new tests use fake-bd",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.97,
            "probabilities": {
              "yes": 0.98,
              "no": 0.02,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.73,
            "probabilities": {
              "yes": 0.82,
              "no": 0.09,
              "unclear": 0.09
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.04,
              "no": 0.95,
              "unclear": 0.01
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.56,
            "probabilities": {
              "yes": 0.71,
              "no": 0.26,
              "unclear": 0.03
            }
          }
        },
        "min_confidence": 0.73,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15382,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60743
      },
      {
        "anchor_id": "REQ-BDC-0001",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0001",
        "statement": "A read of a cached bead must be answered from the local SQLite file without starting a bd process.",
        "quote": "A read of a cached bead must be answered from the local SQLite file without starting a bd process.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 3,
        "deterministic_check": "test_miss_fetches_once_then_serves_from_cache passes; a read during a running sync or hook test starts no bd process",
        "kind": "prohibition",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "yes": 0.02,
              "no": 0.98,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          }
        },
        "min_confidence": 0.97,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15383,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60762
      },
      {
        "anchor_id": "REQ-BDC-0006",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0006",
        "statement": "beads-cache must reach the beads backend only through bd and must never read Dolt tables directly.",
        "quote": "beads-cache must reach the beads backend only through bd and must never read Dolt tables directly.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "grep finds no dolt or mysql client dependency or SQL against Dolt tables in crates/",
        "kind": "prohibition",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.79,
            "probabilities": {
              "yes": 0.86,
              "no": 0.13,
              "unclear": 0.01
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "yes": 0.96,
              "no": 0.02,
              "unclear": 0.02
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.61,
            "probabilities": {
              "yes": 0.74,
              "no": 0.24,
              "unclear": 0.02
            }
          }
        },
        "min_confidence": 0.79,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15378,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60747
      },
      {
        "anchor_id": "REQ-BDC-0007",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0007",
        "statement": "Every bead row stored by beads-cache, including rows ingested from a consumer sync, must pass the strict row decoder generated from bd schema.",
        "quote": "Every bead row stored by beads-cache, including rows ingested from a consumer sync, must pass the strict row decoder generated from bd schema.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 3,
        "deterministic_check": "crates/sc-beads/tests/strict_rows.rs::a_bad_row_is_a_contract_error_naming_bead_and_field rejects an unknown bd-row key as BD_CACHE.BD_CONTRACT naming the bead and field; crates/bd-cache/tests/ingest.rs::one_invalid_row_among_valid_ones_stores_none_of_them rejects an ingest row with an unknown key as INGEST_INVALID (row_invalid, row index) and stores no rows, per approved REQ-BDC-0726 and REQ-BDC-0729.",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.76,
            "probabilities": {
              "yes": 0.84,
              "no": 0.07,
              "unclear": 0.09
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.5,
            "probabilities": {
              "yes": 0.67,
              "no": 0.28,
              "unclear": 0.05
            }
          }
        },
        "min_confidence": 0.76,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15391,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60810
      },
      {
        "anchor_id": "REQ-BDC-0008",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0008",
        "statement": "A sync, fetch or ingest whose result contains one row that fails validation must store no row from that result.",
        "quote": "A sync, fetch or ingest whose result contains one row that fails validation must store no row from that result.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "test_contract_violation_is_typed_and_sync_stores_nothing passes; an ingest_scope test with one bad row leaves the stored scope unchanged",
        "kind": "directive",
        "leaning": "asserted_only",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.95,
              "no": 0.03,
              "unclear": 0.02
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "yes": 0.02,
              "no": 0.98,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.25,
            "probabilities": {
              "yes": 0.42,
              "no": 0.5,
              "unclear": 0.08
            }
          }
        },
        "min_confidence": 0.92,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15385,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60779
      },
      {
        "anchor_id": "REQ-BDC-0502",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0502",
        "statement": "A sync or ingest must not replace a stored bead row whose updated_at is newer than the incoming row.",
        "quote": "A sync or ingest must not replace a stored bead row whose updated_at is newer than the incoming row.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "test_sync_is_incremental_and_never_replaces_a_newer_row passes; an ingest_scope test with an older row keeps the newer stored row",
        "kind": "directive",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.85,
            "probabilities": {
              "yes": 0.9,
              "no": 0.04,
              "unclear": 0.06
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.31,
            "probabilities": {
              "yes": 0.54,
              "no": 0.43,
              "unclear": 0.03
            }
          }
        },
        "min_confidence": 0.85,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15383,
          "output_tokens": 152
        },
        "attempts": 1,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60762
      },
      {
        "anchor_id": "REQ-BDC-0812",
        "source_type": "REQ",
        "source_ref": "REQ-BDC-0812",
        "statement": "New Python binding methods must leave every 0.1 verb, argument and returned dict unchanged and must not issue a DeprecationWarning.",
        "quote": "New Python binding methods must leave every 0.1 verb, argument and returned dict unchanged and must not issue a DeprecationWarning.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 2,
        "deterministic_check": "pytest -W error::DeprecationWarning passes and the 0.1 call corpus test is unchanged",
        "kind": "prohibition",
        "leaning": "satisfied",
        "question_set": {
          "gate": "dev",
          "version": 1,
          "overridden": false
        },
        "questions_sha256": "3c3003769e83070c2318361583df704621657784f6dfbe6c77a894e380f08503",
        "near_threshold": true,
        "confirmed": true,
        "verdict": "satisfied",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.89,
            "probabilities": {
              "yes": 0.93,
              "no": 0.07,
              "unclear": 0.0
            }
          },
          "implemented": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.52,
            "probabilities": {
              "yes": 0.68,
              "no": 0.15,
              "unclear": 0.17
            }
          },
          "violated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.88,
            "probabilities": {
              "yes": 0.05,
              "no": 0.92,
              "unclear": 0.03
            }
          },
          "evidenced": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.62,
            "probabilities": {
              "yes": 0.75,
              "no": 0.23,
              "unclear": 0.02
            }
          }
        },
        "min_confidence": 0.88,
        "explanation": "The directive was followed and evidenced.",
        "usage": {
          "input_tokens": 15392,
          "output_tokens": 152
        },
        "attempts": 2,
        "item_chars_sent": 56116,
        "item_shortened": false,
        "request_bytes": 60802
      },
      {
        "anchor_id": "ADR-BDC-0009",
        "source_type": "ADR",
        "source_ref": "ADR-BDC-0009",
        "statement": "The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface.",
        "quote": "The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface.",
        "stated_by": "randlee",
        "stated_at": "2026-10-25",
        "context": "planning session",
        "anchor_version": 4,
        "deterministic_check": "contract.md section 1 lists both groups; bdc --help shows the beads and cache command groups",
        "kind": "directive",
        "leaning": null,
        "verdict": "excluded",
        "answers": null,
        "min_confidence": null,
        "exclusion": {
          "item": "c-1",
          "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
        },
        "explanation": "Excluded from this work by the anchor's exclude list: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.",
        "attempts": 0
      }
    ],
    "deterministic_checks": [
      {
        "anchor_id": "ADR-BDC-0005",
        "verdict": "satisfied",
        "check": "the outage tests show reads still answered and the scope's outage recorded on spawn failure, timeout and transport errors"
      },
      {
        "anchor_id": "ADR-STORE-0001",
        "verdict": "satisfied",
        "check": "the format golden test matches the exact v3 DDL; PRAGMA user_version stays 3"
      },
      {
        "anchor_id": "ANC-001",
        "verdict": "satisfied",
        "check": "`python3 scripts/lint_no_atm.py` exits 0 on the whole tree; per sprint, `python3 scripts/lint_no_atm.py --paths <sprint fence>` exits 0"
      },
      {
        "anchor_id": "ANC-002",
        "verdict": "satisfied",
        "check": "a test runs the built-in sync from an sc-cron schedule row in the cache file"
      },
      {
        "anchor_id": "ANC-003",
        "verdict": "satisfied",
        "check": "an end-to-end test disables the built-in sync row and a registered Python sync ingests a scope"
      },
      {
        "anchor_id": "ANC-004",
        "verdict": "satisfied",
        "check": "an end-to-end test asserts the post-sync hook request carries each scope's result and error"
      },
      {
        "anchor_id": "ANC-008",
        "verdict": "satisfied",
        "check": "a test shows a second built-in sync row is refused while other rows are allowed"
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "satisfied",
        "check": "tests sync a label-filter scope and an --all scope; grep finds no phase-file reading in crates/"
      },
      {
        "anchor_id": "NFR-BDC-0004",
        "verdict": "satisfied",
        "check": "a test with a hanging fake bd and a hanging hook shows each is killed at its timeout and the run ends by its deadline"
      },
      {
        "anchor_id": "NFR-BDC-0005",
        "verdict": "satisfied",
        "check": "CI unit jobs run with no bd on PATH; new tests use fake-bd"
      },
      {
        "anchor_id": "REQ-BDC-0001",
        "verdict": "satisfied",
        "check": "test_miss_fetches_once_then_serves_from_cache passes; a read during a running sync or hook test starts no bd process"
      },
      {
        "anchor_id": "REQ-BDC-0006",
        "verdict": "satisfied",
        "check": "grep finds no dolt or mysql client dependency or SQL against Dolt tables in crates/"
      },
      {
        "anchor_id": "REQ-BDC-0007",
        "verdict": "satisfied",
        "check": "crates/sc-beads/tests/strict_rows.rs::a_bad_row_is_a_contract_error_naming_bead_and_field rejects an unknown bd-row key as BD_CACHE.BD_CONTRACT naming the bead and field; crates/bd-cache/tests/ingest.rs::one_invalid_row_among_valid_ones_stores_none_of_them rejects an ingest row with an unknown key as INGEST_INVALID (row_invalid, row index) and stores no rows, per approved REQ-BDC-0726 and REQ-BDC-0729."
      },
      {
        "anchor_id": "REQ-BDC-0008",
        "verdict": "satisfied",
        "check": "test_contract_violation_is_typed_and_sync_stores_nothing passes; an ingest_scope test with one bad row leaves the stored scope unchanged"
      },
      {
        "anchor_id": "REQ-BDC-0502",
        "verdict": "satisfied",
        "check": "test_sync_is_incremental_and_never_replaces_a_newer_row passes; an ingest_scope test with an older row keeps the newer stored row"
      },
      {
        "anchor_id": "REQ-BDC-0812",
        "verdict": "satisfied",
        "check": "pytest -W error::DeprecationWarning passes and the 0.1 call corpus test is unchanged"
      }
    ],
    "evidence_sections": [
      "Sprint report",
      "Files changed",
      "Tests"
    ],
    "item_truncated": false,
    "shortened": [],
    "report_markdown": "## Anchor gate: dev · c-1 - runtime-migration\n\nRun `dev-20261107T002343Z-c-1`, model `jev-1.14.0`, threshold 0.6. Counts: satisfied 16, excluded 1.\n\n> **c-1 · ADR-BDC-0009 · excluded** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Not judged: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.\n\nAll judged anchors are satisfied or not applicable.\n\n",
    "qa15_to_qa16_diff": {
      "prior_run_id": "dev-20261106T235757Z-c-1",
      "prior_summary": {
        "counts": {
          "asserted_only": 1,
          "underspecified": 1,
          "satisfied": 14,
          "excluded": 1
        },
        "failing": [
          "ANC-008",
          "ANC-001"
        ],
        "leaning_counts": {
          "asserted_only": 3,
          "not_addressed": 1,
          "satisfied": 12
        },
        "unstable": [],
        "errors": [],
        "excluded": [
          {
            "anchor_id": "ADR-BDC-0009",
            "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
          }
        ],
        "shortened": [],
        "pass": false
      },
      "current_run_id": "dev-20261107T002343Z-c-1",
      "current_summary": {
        "counts": {
          "satisfied": 16,
          "excluded": 1
        },
        "failing": [],
        "leaning_counts": {
          "asserted_only": 3,
          "satisfied": 13
        },
        "unstable": [],
        "errors": [],
        "excluded": [
          {
            "anchor_id": "ADR-BDC-0009",
            "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
          }
        ],
        "shortened": [],
        "pass": true
      },
      "changes": "ANC-008 moved asserted_only→satisfied after applying the independently accepted implemented-only override; ANC-001 moved underspecified→satisfied after the appended report explicitly named both whole-tree and sprint-path lint commands/output. ANC-004 remained satisfied; the other applicable anchors stayed satisfied and ADR-BDC-0009 remained excluded.",
      "item_growth": "QA15 item was 58,243 chars; QA16 is 56,116 chars (2,127 fewer). The full normative sprint prefix is unchanged; the decrease is confined to the corrected appended report."
    },
    "anc008_calibration_and_application": {
      "accepted_candidate_id": "c03",
      "accepted_tournament_result": "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/accepted-result.json",
      "accepted_semantic_score": {
        "exact_role_cells": 112,
        "total_role_cells": 112,
        "exact_semantic_verdicts": 28,
        "total_semantic_verdicts": 28,
        "stable_cases": 14,
        "total_cases": 14,
        "regressions": 0
      },
      "approved_production_fixtures": "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/production-fixture-check.json",
      "production_fixture_summary": {
        "judged": 2,
        "pass": true,
        "mismatches": 0
      },
      "question_bead_application": "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/question-application-receipt.json",
      "accepted_sealed_questions_sha256": "c4afc2cef0d43f95d0cd3ded23352fde1c2abf4fb76e79380355ddf3cf27913d",
      "effective_run_questions_sha256": "0513e743fd340219e1a4fe960e27da347688718cc6dcba5dc12be56b8da1903b",
      "changed_role": "ANC-008 implemented only; remaining roles are the unchanged generic dev v1 questions",
      "remaining_question_overrides": "ANC-004 and NFR-BDC-0004 retained unchanged; all other question hashes match default dev v1"
    },
    "paired_deterministic_check_summary": {
      "source": "/tmp/phase-c-execution/c-1/qa-15/rust-qa-deterministic-checks.json",
      "summary": {
        "applicable": 16,
        "recorded_pass": 16,
        "pending": 0,
        "excluded": 1
      },
      "note": "Independent current-source check receipt; it supports the 16 listed checks but is distinct from the Jev verdict."
    },
    "limits": [
      "This is the development-gate result for the exact submitted item at threshold 0.6; it is not whole-project QA or sprint-merge approval.",
      "No external calls, retries, source edits, or question edits were made during this analysis. The approved ANC-008 implemented-only override was already applied by the parent and is reported with its accepted provenance."
    ],
    "source_sha256": {
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-16-anchor-evidence/runs/dev-20261107T002343Z-c-1.json": "60c1a383472856d7ab2fa009c01dcdaf11d7295ae5d869825c7b09b8ed8a3885",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-15-anchor-evidence/runs/dev-20261106T235757Z-c-1.json": "13c1f164625ef46879d97fff942d7d8c19b770147c6b626e66431c3c7dd389e7",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-16-anchor-evidence/item.txt": "fa73c8f561e66dae9b372e6248cd5e3e32966072c3b3223d71c4ff9b95f6c1fc",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/target/qa-16-anchor-evidence/corrected-sprint-report.md": "ef726a72285b8ca1ed65d6aebbdbde3551773e46855c823bbe290dbd0bccb0e4",
      "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration/docs/plans/phase-c/anchors/anchors.json": "19f70fcfd8ed07bec7cb04d1cfc6fabddadd21d537ca528ec937c5de2452f702",
      "/tmp/phase-c-execution/c-1/qa-16/anchor-runtime.json": "40ea691ad96e693097a3125d387e28b7b6e6af401207e2e4d6448e4c3c5b401d",
      "/tmp/phase-c-execution/c-1/qa-16/anchors/live-stdout.json": "11cb2772bf41e3764a5514f58d032138081b95b4a38fa66378adfbc188af0bad",
      "/tmp/phase-c-execution/c-1/qa-15/rust-qa-deterministic-checks.json": "63f2228b2dcb32be5f0214cb5e7fd11228f1996e0185d9618df324be09cc9012",
      "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/accepted-result.json": "a5e063180a0cad81018a10701b75cfc70ca799f3d1aeec9eec37672b7e39207c",
      "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/question-application-receipt.json": "859875facf170edfaa140cef60c49a54feb663216fad283795b49f8a14a31029",
      "/tmp/phase-c-execution/c-1/qa-15/anc008-calibration/production-fixture-check.json": "67690dc51301fe1e92682ef8bc87bbb3ba8d9ef3f378e36643d3e90d0bf3130f"
    }
  },
  "error": null,
  "metadata": {
    "correlation_id": "c-1-qa16"
  }
}
```
