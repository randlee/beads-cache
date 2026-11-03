```json
{
  "schema_version": "1.0.0",
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "c-1",
    "run_id": "plan-20261103T000924Z-c-1",
    "run_path": "docs/plans/phase-c/anchors/runs/plan-20261103T000924Z-c-1.json",
    "dry_run": false,
    "reused_run": true,
    "pass": false,
    "failing": [
      "ADR-BDC-0009"
    ],
    "errors": [],
    "excluded": [],
    "results": [
      {
        "anchor_id": "ADR-BDC-0009",
        "verdict": "underspecified",
        "leaning": "not_applicable",
        "min_confidence": 0.56,
        "attempts": 2,
        "unstable": true,
        "explanation": "Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.64,
            "probabilities": {
              "no": 0.76,
              "unclear": 0.0,
              "yes": 0.24
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.01,
              "yes": 0.01
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        },
        "low_questions": [
          {
            "ask": "second",
            "role": "relevance",
            "question_id": "relevant",
            "choice": "no",
            "confidence": 0.56
          }
        ],
        "answers_second": {
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.56,
            "probabilities": {
              "no": 0.71,
              "unclear": 0.0,
              "yes": 0.29
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.02,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        },
        "verdict_first": "not_applicable",
        "verdict_second": "underspecified",
        "near_threshold": true,
        "to_fix": {
          "where": null,
          "change": "exclude --id ADR-BDC-0009 --item c-1 --reason \"c-1 migrates scheduler dependencies and internals while preserving the existing public facade; it changes no bdc command surface or bd-cache-api request vocabulary.\""
        }
      },
      {
        "anchor_id": "ADR-BDC-0005",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ADR-STORE-0001",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.96,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.96,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.02,
              "yes": 0.97
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.01,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-002",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.97,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.02,
              "unclear": 0.0,
              "yes": 0.98
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-003",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.04,
              "unclear": 0.01,
              "yes": 0.95
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-004",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.76,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.76,
            "probabilities": {
              "no": 0.16,
              "unclear": 0.0,
              "yes": 0.84
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.08,
              "unclear": 0.0,
              "yes": 0.92
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-008",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.96,
              "unclear": 0.01,
              "yes": 0.03
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.01,
              "yes": 0.04
            }
          }
        }
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.83,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.83,
            "probabilities": {
              "no": 0.1,
              "unclear": 0.01,
              "yes": 0.89
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "NFR-BDC-0004",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "NFR-BDC-0005",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.91,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.91,
            "probabilities": {
              "yes": 0.94,
              "no": 0.06,
              "unclear": 0.0
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0001",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0006",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.95,
              "no": 0.05,
              "unclear": 0.0
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.01,
              "no": 0.99,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0007",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0008",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0502",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "yes": 1.0,
              "no": 0.0,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "yes": 0.0,
              "no": 1.0,
              "unclear": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "REQ-BDC-0812",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.96,
            "probabilities": {
              "yes": 0.97,
              "no": 0.03,
              "unclear": 0.0
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.58,
            "probabilities": {
              "yes": 0.72,
              "no": 0.2,
              "unclear": 0.08
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "yes": 0.04,
              "no": 0.95,
              "unclear": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-001",
        "verdict": "deterministic_only",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Structural anchor: verified by its deterministic_check, not by Jev",
        "answers": null
      }
    ],
    "overrides_applied": [
      "ADR-BDC-0009",
      "ADR-STORE-0001",
      "ANC-002",
      "ANC-003",
      "ANC-004",
      "ANC-008",
      "ANC-009",
      "REQ-BDC-0001"
    ],
    "item_truncated": false,
    "shortened": [],
    "report_markdown": "## Anchor gate: plan · c-1 - sprint-c-1-runtime-migration\n\nRun `plan-20261103T000924Z-c-1`, model `jev-1.14.0`, threshold 0.6. Counts: underspecified 1, satisfied 15, deterministic_only 1.\n\n> **c-1 · ADR-BDC-0009 · underspecified** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified. (leaning: not_applicable)\n>   Low: relevance `relevant` answered no at 0.56 (second ask).\n>   Unstable: two identical requests gave not_applicable then underspecified.\n\n"
  },
  "error": null,
  "metadata": {
    "duration_ms": 58,
    "tool_calls": 5,
    "retry_count": 1,
    "correlation_id": "phase-c-postqa1-c1-initial"
  }
}
```
