# Production-style warehouse lineage sample

Synthetic but realistic multi-layer platform for demos:

`raw → staging → intermediate → marts → metrics → serving` (+ reverse ETL, orphans).

## Quick steps

From the **repo root** (needs [Rust](https://rustup.rs/)):

```bash
# 1) Load this sample into a local store
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  import examples/production_warehouse_lineage \
  --label prod-demo

# 2) Write an offline HTML report
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  export -f html -o lineage.html

# 3) Open it in a browser (no server)
open lineage.html
# Linux: xdg-open lineage.html
```

Also on the main README under **Quick usage**.