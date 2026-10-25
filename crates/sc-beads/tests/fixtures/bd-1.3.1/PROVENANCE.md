# bd 1.3.1 contract fixtures

Recorded output of bd 1.3.1, the acceptance fixtures of the v3 file format
(REQ-BDC-0804, ADR-STORE-0001).

| File | What | Check |
|---|---|---|
| `bd-schema.json` | full `bd schema` stdout of bd 1.3.1 | sha256 `caf8fe18e1117c067ff7bbd824484b24849677e2cbf86d793ddf6b64069a31a2` equals `BD_SCHEMA_SHA256` (`test_pinned_model_matches_recorded_schema`) |
| `list-rows.json` | three rows recorded from `bd list --json` (bd 1.3.1, database `ct`) | every row validates (`test_recorded_bd_rows_validate`) |

The strict model `src/row/generated.rs` is generated from `bd-schema.json`
(`SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated`).
Do not edit these files; re-vendor them for a new bd release.
