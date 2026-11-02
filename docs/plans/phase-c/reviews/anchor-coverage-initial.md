## Anchor coverage (plan)

| Anchor | c-1 | c-2 | c-3 | c-4 | Covered |
| --- | --- | --- | --- | --- | --- |
| ADR-BDC-0005 | satisfied | satisfied | not applicable | not applicable | yes |
| ADR-BDC-0009 | not applicable | excluded | not applicable | not applicable | no |
| ADR-STORE-0001 | satisfied | not applicable | not applicable | not applicable | yes |
| ANC-001 | deterministic only | deterministic only | deterministic only | deterministic only | no |
| ANC-002 | satisfied | satisfied | not applicable | not applicable | yes |
| ANC-003 | satisfied | satisfied | not applicable | not applicable | yes |
| ANC-004 | satisfied | excluded | not applicable | not applicable | yes |
| ANC-008 | satisfied | satisfied | not applicable | not applicable | yes |
| ANC-009 | satisfied | excluded | not applicable | not applicable | yes |
| NFR-BDC-0004 | satisfied | satisfied | not applicable | not applicable | yes |
| NFR-BDC-0005 | satisfied | satisfied | excluded | excluded | yes |
| REQ-BDC-0001 | satisfied | satisfied | not applicable | not applicable | yes |
| REQ-BDC-0006 | satisfied | satisfied | not applicable | not applicable | yes |
| REQ-BDC-0007 | satisfied | satisfied | not applicable | not applicable | yes |
| REQ-BDC-0008 | satisfied | satisfied | not applicable | not applicable | yes |
| REQ-BDC-0502 | satisfied | satisfied | not applicable | not applicable | yes |
| REQ-BDC-0812 | satisfied | satisfied | not applicable | excluded | yes |

Uncovered anchors (no item satisfies them): ADR-BDC-0009, ANC-001

Excluded cells (not judged):
- ADR-BDC-0009 x c-2: c-2 adds service/daemon assembly and health only; it adds or changes no bdc command or bd-cache-api request vocabulary.
- ANC-004 x c-2: c-2 only hosts/invokes the existing sync and hook; it changes no sync job or post-sync hook behavior, which c-1 owns. The anchor scope explicitly excludes invocation-only work.
- ANC-009 x c-2: c-2 only hosts the existing sync; it defines/parses no scopes and changes no scope-filter sync implementation. c-1 qualifies that behavior.
- NFR-BDC-0005 x c-3: c-3 changes template-regeneration helper tests only, not tests of sync jobs, hooks, ingest or CLI behavior named in this anchor scope.
- NFR-BDC-0005 x c-4: c-4 changes packaging/checksum helper tests and installed-artifact integration checks, not unit tests of sync jobs, hooks, ingest or CLI behavior.
- REQ-BDC-0812 x c-4: c-4 changes packaging and artifact validation, not Python binding methods, arguments or returned values; c-1/c-2 own compatibility.
