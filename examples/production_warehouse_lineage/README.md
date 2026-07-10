# Production-style warehouse lineage sample

Synthetic but realistic multi-layer platform for demos:

`raw → staging → intermediate → marts → metrics → serving` (+ reverse ETL, orphans).

```bash
cargo run -q -p simplineage-cli -- --data-dir /tmp/prod-lineage --no-progress \
  import examples/production_warehouse_lineage --label prod-demo

cargo run -q -p simplineage-cli -- --data-dir /tmp/prod-lineage --no-progress \
  export -f html -o /tmp/prod-lineage.html
```
