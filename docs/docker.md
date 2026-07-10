# Running SimpLineage with Docker

You can use SimpLineage **without installing Rust** on your machine. Only
[Docker](https://docs.docker.com/get-docker/) (Desktop, Colima, OrbStack, etc.) is required.

Two images:

| Image | File | Purpose |
|-------|------|---------|
| **Runtime CLI** | `Dockerfile` | Run `simplineage` (import, export, analysis) |
| **Dev tools** | `Dockerfile.dev` | Full Rust toolchain for contributors (fmt, clippy, nextest, …) |

---

## Runtime CLI (recommended for users)

### Build

From the repo root:

```bash
docker compose build simplineage
# equivalent: docker build -t simplineage:local .
```

### Smoke test

```bash
docker compose run --rm simplineage
# prints a hello message
```

### Pass any CLI command

Arguments after the service name go to `simplineage`:

```bash
docker compose run --rm simplineage --help
docker compose run --rm simplineage version
docker compose run --rm simplineage hello --name engineer
```

### Import sample data and open an HTML report

The runtime image does **not** bundle `examples/`. Mount the repo examples (read-only)
and a host folder for output. Use a **named volume** for the metadata store so it
survives container restarts.

```bash
mkdir -p ./out

# 1) Import production-style sample
docker compose run --rm \
  -v "$(pwd)/examples:/data/examples:ro" \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  import /data/examples/production_warehouse_lineage \
  --label prod-demo

# 2) Export offline interactive HTML
docker compose run --rm \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  export -f html -o /data/out/lineage.html

# 3) Open on the host (no server)
open ./out/lineage.html          # macOS
# xdg-open ./out/lineage.html    # Linux
```

Smaller BigQuery-style sample — same pattern, different import path:

```bash
docker compose run --rm \
  -v "$(pwd)/examples:/data/examples:ro" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  import /data/examples/bigquery_information_schema \
  --label bq-demo
```

### Import your own files

```bash
docker compose run --rm \
  -v "/path/to/your/export:/data/in:ro" \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  import /data/in

docker compose run --rm \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  export -f html -o /data/out/lineage.html
```

### Plain `docker run` (no Compose)

```bash
docker build -t simplineage:local .

docker run --rm \
  -v "$(pwd)/examples:/data/examples:ro" \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage:local \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  import /data/examples/production_warehouse_lineage --label prod-demo

docker run --rm \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage:local \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  export -f html -o /data/out/lineage.html
```

### Tips

| Topic | Detail |
|--------|--------|
| Default store path | `/home/simplineage/.simplineage` (also `SIMPLINEAGE_STORAGE__DATA_DIR` in the image) |
| Persist store | Mount a named volume on that path |
| HTML on host | Write under a bind-mounted dir (e.g. `./out`) so the browser can open the file |
| User | Container runs as non-root user `simplineage` |
| After `git pull` | Rebuild: `docker compose build simplineage` |

More CLI flags: [cli.md](cli.md). HTML report features: [html-export.md](html-export.md).

---

## Dev container (contributors)

Full toolchain without installing `cargo-nextest`, `cargo-deny`, etc. on the host:

```bash
docker compose --profile dev run --rm dev
# shell inside /workspace (repo bind-mounted)
cargo test --workspace
cargo run -p simplineage-cli -- --help
```

Or:

```bash
docker build -f Dockerfile.dev -t simplineage-dev .
docker run --rm -it -v "$(pwd):/workspace" -w /workspace simplineage-dev bash
```

See also [development.md](development.md) and [CONTRIBUTING.md](../CONTRIBUTING.md).
