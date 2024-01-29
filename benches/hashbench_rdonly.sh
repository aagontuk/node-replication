#!/bin/bash
set -ex

cargo build --release
rm results.log || true
for a in local remote; do
  for d in uniform skewed; do
    for r in 1 4 8; do
      if [[ $w > 0 ]] || [[ $r > 0 ]] ; then
        if [[ $a == "local" ]]; then
          RUST_TEST_THREADS=1 numactl --cpunodebind=0 --membind=0 -- cargo bench --bench hashbench --features="nr" -- -c "std" -r $r -w 0 -d $d -a "local" | tee -a results.log;
        else
          RUST_TEST_THREADS=1 numactl --cpunodebind=0 --membind=1 -- cargo bench --bench hashbench --features="nr" -- -c "std" -r $r -w 0 -d $d -a "remote" | tee -a results.log;
        fi
      fi
    done;
  done;
done;

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
R -q --no-readline --no-restore --no-save < $SCRIPT_DIR/hashbench_plot.r
