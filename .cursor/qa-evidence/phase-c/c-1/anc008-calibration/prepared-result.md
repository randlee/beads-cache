```json
{
  "schema_version": "1.0.0",
  "success": false,
  "canceled": false,
  "aborted_by": null,
  "data": null,
  "error": {"code":"EVIDENCE.INCOMPLETE","message":"First-stage preparation complete; awaiting parent baseline transport. Oracle frozen before any Jev response: 9e2127663424aeb7a54a7b9daaa5fc8c001cba4e0aaca5f1b6faae9d8377a4ce. Fourteen authority-only cases, exact expected roles and installed threshold-0.0 verdicts preserved; no tournament outcome, override, or seal exists.","recoverable":true,"suggested_action":"Transport only c00 via /tmp/phase-c-execution/c-1/qa-15/anc008-calibration/baseline/manifest.json (28 requests, 2 identical repeats per case, max parallel 4, 30-second timeout, one recoverable retry). Authority bindings: /tmp/phase-c-execution/c-1/qa-15/anc008-calibration/authority-bindings.json. Freeze receipt/oracle and 3 candidate bundles are retained in the same directory. Use manifest.evaluation_argv after preserving raw responses. Do not transport alternative candidates unless baseline demonstrates a real defect; do not relabel the oracle. Production threshold remains 0.6; confidence reference 0.8 remains diagnostic."},
  "metadata": {"authoritative_sources_unchanged":true,"correlation_id":"anc008-development-independent-calibration","duration_ms":0,"jev_run_ids":[],"planner_observation_trust":null,"retry_count":0,"tool_calls":2}
}
```