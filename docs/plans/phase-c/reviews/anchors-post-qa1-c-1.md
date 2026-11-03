```json
{
  "schema_version": "1.0.0",
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "c-1",
    "run_id": "plan-20261103T001057Z-c-1",
    "run_path": "docs/plans/phase-c/anchors/runs/plan-20261103T001057Z-c-1.json",
    "dry_run": false,
    "reused_run": true,
    "pass": true,
    "failing": [],
    "errors": [],
    "excluded": [
      "ADR-BDC-0009"
    ],
    "results": [
      {
        "anchor_id": "ADR-BDC-0005",
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
              "yes": 0.98,
              "no": 0.02,
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
        "min_confidence": 0.97,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.01,
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
        "min_confidence": 0.79,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.79,
            "probabilities": {
              "no": 0.14,
              "unclear": 0.0,
              "yes": 0.86
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
        "min_confidence": 0.89,
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.01,
              "yes": 0.02
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.93,
              "unclear": 0.01,
              "yes": 0.06
            }
          }
        }
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.88,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.07,
              "unclear": 0.01,
              "yes": 0.92
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
        "min_confidence": 0.85,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.85,
            "probabilities": {
              "yes": 0.9,
              "no": 0.09,
              "unclear": 0.01
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
        "min_confidence": 1.0,
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
        "min_confidence": 0.88,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.88,
            "probabilities": {
              "yes": 0.92,
              "no": 0.08,
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
        "min_confidence": 0.89,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
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
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.55,
            "probabilities": {
              "yes": 0.7,
              "no": 0.19,
              "unclear": 0.11
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.89,
            "probabilities": {
              "yes": 0.06,
              "no": 0.93,
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
      },
      {
        "anchor_id": "ADR-BDC-0009",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.",
        "answers": null,
        "exclusion": {
          "item": "c-1",
          "reason": "c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope."
        }
      }
    ],
    "overrides_applied": [
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
    "report_markdown": "## Anchor gate: plan · c-1 - sprint-c-1-runtime-migration\n\nRun `plan-20261103T001057Z-c-1`, model `jev-1.14.0`, threshold 0.6. Counts: satisfied 15, deterministic_only 1, excluded 1.\n\n> **c-1 · ADR-BDC-0009 · excluded** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Not judged: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.\n\nAll judged anchors are satisfied or not applicable.\n\n"
  },
  "error": null,
  "metadata": {
    "duration_ms": 70,
    "tool_calls": 4,
    "retry_count": 0,
    "correlation_id": "phase-c-postqa1-c-1"
  }
}
```
