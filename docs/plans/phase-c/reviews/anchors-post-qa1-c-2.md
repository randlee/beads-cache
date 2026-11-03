```json
{
  "schema_version": "1.0.0",
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "c-2",
    "run_id": "plan-20261103T000925Z-c-2",
    "run_path": "docs/plans/phase-c/anchors/runs/plan-20261103T000925Z-c-2.json",
    "dry_run": false,
    "reused_run": true,
    "pass": true,
    "failing": [],
    "errors": [],
    "excluded": [
      "ADR-BDC-0009",
      "ANC-004",
      "ANC-009"
    ],
    "results": [
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
        "anchor_id": "ANC-002",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.96,
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.0,
              "yes": 0.03
            }
          }
        }
      },
      {
        "anchor_id": "ANC-003",
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
        "anchor_id": "ANC-008",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.8,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.8,
            "probabilities": {
              "no": 0.12,
              "unclear": 0.01,
              "yes": 0.87
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
            "confidence": 0.98,
            "probabilities": {
              "yes": 0.99,
              "no": 0.01,
              "unclear": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "yes": 0.02,
              "no": 0.98,
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
        "anchor_id": "REQ-BDC-0006",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.89,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.89,
            "probabilities": {
              "yes": 0.93,
              "no": 0.06,
              "unclear": 0.01
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.37,
            "probabilities": {
              "yes": 0.58,
              "no": 0.33,
              "unclear": 0.09
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
        "anchor_id": "REQ-BDC-0502",
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
        "anchor_id": "REQ-BDC-0812",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.86,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.86,
            "probabilities": {
              "yes": 0.91,
              "no": 0.09,
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
        "anchor_id": "ADR-STORE-0001",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.04,
              "yes": 0.01
            }
          },
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.83,
            "probabilities": {
              "no": 0.89,
              "unclear": 0.01,
              "yes": 0.1
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
        "explanation": "Excluded from this bead by the anchor's exclude list: c-2 adds service/daemon assembly and health only; it adds or changes no bdc command or bd-cache-api request vocabulary.",
        "answers": null,
        "exclusion": {
          "item": "c-2",
          "reason": "c-2 adds service/daemon assembly and health only; it adds or changes no bdc command or bd-cache-api request vocabulary."
        }
      },
      {
        "anchor_id": "ANC-004",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: c-2 only hosts/invokes the existing sync and hook; it changes no sync job or post-sync hook behavior, which c-1 owns. The anchor scope explicitly excludes invocation-only work.",
        "answers": null,
        "exclusion": {
          "item": "c-2",
          "reason": "c-2 only hosts/invokes the existing sync and hook; it changes no sync job or post-sync hook behavior, which c-1 owns. The anchor scope explicitly excludes invocation-only work."
        }
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: c-2 only hosts the existing sync; it defines/parses no scopes and changes no scope-filter sync implementation. c-1 qualifies that behavior.",
        "answers": null,
        "exclusion": {
          "item": "c-2",
          "reason": "c-2 only hosts the existing sync; it defines/parses no scopes and changes no scope-filter sync implementation. c-1 qualifies that behavior."
        }
      }
    ],
    "overrides_applied": [
      "ANC-002",
      "ANC-003",
      "ANC-008",
      "REQ-BDC-0001",
      "ADR-STORE-0001"
    ],
    "item_truncated": false,
    "shortened": [],
    "report_markdown": "## Anchor gate: plan · c-2 - sprint-c-2-framework-application\n\nRun `plan-20261103T000925Z-c-2`, model `jev-1.14.0`, threshold 0.6. Counts: satisfied 12, not_applicable 1, deterministic_only 1, excluded 3.\n\n> **c-2 · ADR-BDC-0009 · excluded** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Not judged: c-2 adds service/daemon assembly and health only; it adds or changes no bdc command or bd-cache-api request vocabulary.\n\n> **c-2 · ANC-004 · excluded** Anchor: beads-cache must support an optional consumer-registered post-sync hook that receives the bd sync results and errors. Not judged: c-2 only hosts/invokes the existing sync and hook; it changes no sync job or post-sync hook behavior, which c-1 owns. The anchor scope explicitly excludes invocation-only work.\n\n> **c-2 · ANC-009 · excluded** Anchor: beads-cache sync scopes must be defined as bd filters, such as labels or --all, instead of as phases. Not judged: c-2 only hosts the existing sync; it defines/parses no scopes and changes no scope-filter sync implementation. c-1 qualifies that behavior.\n\nAll judged anchors are satisfied or not applicable.\n\n"
  },
  "error": null,
  "metadata": {
    "duration_ms": 65,
    "tool_calls": 4,
    "retry_count": 0,
    "correlation_id": "phase-c-postqa1-c-2"
  }
}
```
