// Copyright © 2019 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Defines a hash-map that can be replicated.
#![feature(test)]

use std::collections::HashMap;

use rand::{distributions::Distribution, Rng, RngCore};
use zipf::ZipfDistribution;

use node_replication::Dispatch;

mod mkbench;
mod utils;

use utils::benchmark::*;
use utils::Operation;

/// Operations we can perform on the stack.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpWr {
    /// Add an item to the hash-map.
    Put(u64, u64),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpRd {
    /// Get item from the hash-map.
    Get(u64),
}

/// Single-threaded implementation of the stack
///
/// We just use a vector.
#[derive(Debug, Clone)]
pub struct NrHashMap {
    storage: HashMap<u64, u64>,
}

impl NrHashMap {
    pub fn put(&mut self, key: u64, val: u64) {
        self.storage.insert(key, val);
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        self.storage.get(&key).map(|v| *v)
    }
}

impl Default for NrHashMap {
    /// Return a dummy hash-map with initial capacity of 50M elements.
    fn default() -> NrHashMap {
        let capacity = 5_000_000;
        let mut storage = HashMap::with_capacity(capacity);
        for i in 0..capacity {
            storage.insert(i as u64, (i + 1) as u64);
        }
        NrHashMap { storage }
    }
}

impl Dispatch for NrHashMap {
    type ReadOperation = OpRd;
    type WriteOperation = OpWr;
    type Response = Option<u64>;
    type ResponseError = ();

    fn dispatch(&self, op: Self::ReadOperation) -> Result<Self::Response, Self::ResponseError> {
        match op {
            OpRd::Get(key) => return Ok(self.get(key)),
        }
    }

    /// Implements how we execute operation from the log against our local stack
    fn dispatch_mut(
        &mut self,
        op: Self::WriteOperation,
    ) -> Result<Self::Response, Self::ResponseError> {
        match op {
            OpWr::Put(key, val) => {
                self.put(key, val);
                Ok(None)
            }
        }
    }
}

/// Generate a random sequence of operations
///
/// # Arguments
///  - `write_ratio`: Probability of generation a write give a value in [0..100]
///  - `span`: Maximum key-space
///  - `distribution`: Supported distribution 'uniform' or 'skewed'
fn generate_operation(
    rng: &mut rand::rngs::SmallRng,
    write_ratio: usize,
    span: usize,
    distribution: &'static str,
) -> Operation<OpRd, OpWr> {
    assert!(distribution == "skewed" || distribution == "uniform");

    let skewed = distribution == "skewed";
    let zipf = ZipfDistribution::new(span, 1.03).unwrap();

    let id = if skewed {
        zipf.sample(rng) as u64
    } else {
        // uniform
        rng.gen_range(0, span as u64)
    };

    if rng.gen::<usize>() % 100 < write_ratio {
        Operation::WriteOperation(OpWr::Put(id, rng.next_u64()))
    } else {
        Operation::ReadOperation(OpRd::Get(id))
    }
}

/// Compare a replicated hashmap against a single-threaded implementation.
fn hashmap_single_threaded(c: &mut TestHarness) {
    // Size of the log.
    const LOG_SIZE_BYTES: usize = 2 * 1024 * 1024;

    mkbench::baseline_comparison::<NrHashMap>(c, "hashmap", LOG_SIZE_BYTES, &mut |rng| {
        // Biggest key in the hash-map
        const KEY_SPACE: usize = 10_000;
        // Key distribution
        const UNIFORM: &'static str = "uniform";
        //const SKEWED: &'static str = "skewed";
        // Read/Write ratio
        const WRITE_RATIO: usize = 10; //% out of 100
        generate_operation(rng, WRITE_RATIO, KEY_SPACE, UNIFORM)
    });
}

/// Compare scale-out behaviour of synthetic data-structure.
fn hashmap_scale_out(c: &mut TestHarness) {
    // Biggest key in the hash-map
    const KEY_SPACE: usize = 5_000_000;
    // Key distribution
    const UNIFORM: &'static str = "uniform";
    //const SKEWED: &'static str = "skewed";
    // Read/Write ratio
    const WRITE_RATIO: usize = 10; //% out of 100

    mkbench::ScaleBenchBuilder::new()
        .machine_defaults()
        .configure::<NrHashMap>(
            c,
            "hashmap-scaleout",
            |_cid, rid, _log, replica, _batch_size, rng| match generate_operation(
                rng,
                WRITE_RATIO,
                KEY_SPACE,
                UNIFORM,
            ) {
                Operation::ReadOperation(op) => {
                    replica.execute_ro(op, rid).unwrap();
                }
                Operation::WriteOperation(op) => {
                    replica.execute(op, rid).unwrap();
                }
            },
        );
}

fn main() {
    let _r = env_logger::try_init();
    let mut harness = Default::default();

    hashmap_single_threaded(&mut harness);
    hashmap_scale_out(&mut harness);
}
