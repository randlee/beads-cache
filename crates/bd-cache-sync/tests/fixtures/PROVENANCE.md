# Fixtures: atm-core phase files

`phase-bd.toml` and `phase-bk.toml` are byte-for-byte copies of
`.atm-bd/phase-bd.toml` and `.atm-bd/phase-bk.toml` in randlee/atm-core at commit
`05831dfe866d389feed138add3c29cb93cd6c82b` (the BD-CACHE baseline), fetched with
`gh api repos/randlee/atm-core/contents/.atm-bd/<file>?ref=<commit>`.

They are the files `bd_cache_cron.py` (`phases()`) reads in production: a phase's
`root` and `lead` are used, every other key (`plan`, `integration_branch`,
`[templates]`) is ignored. `tests/tick_phases.rs::the_phase_files_atm_core_keeps_are_read`
loads them. `.gitattributes` keeps them LF.
