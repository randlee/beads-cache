# bd-cache-model fixtures

`python-journal-args.json`: journal `args_json` values written the way the
BD-CACHE v3 reference implementation (the v3 Python writer's journal write)
writes them: `json.dumps(args, sort_keys=True)`,
generated with CPython 3.14 for sprint a-1. Each entry is an op name and the
exact stored text; `Op::parse` then `Op::to_args_json` must reproduce it byte
for byte.
