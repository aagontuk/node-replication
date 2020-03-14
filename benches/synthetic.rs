// Copyright © 2019 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Defines a synthethic data-structure that can be replicated.
//!
//! The data-structure is configurable with 4 parameters: cold_reads, cold_writes, hot_reads, hot_writes
//! which simulates how many cold/random and hot/cached cache-lines are touched for every operation.
//!
//! It evaluates the overhead of the log with an abstracted model of a generic data-structure
//! to measure the cache-impact.

#![feature(test)]

use crossbeam_utils::CachePadded;
use rand::Rng;

use node_replication::Dispatch;

mod mkbench;
mod utils;

use utils::benchmark::*;
use utils::Operation;

/// Operations we can perform on the AbstractDataStructure.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpRd {
    /// Read a bunch of local memory.
    ReadOnly(usize, usize, usize),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpWr {
    /// Write a bunch of local memory.
    WriteOnly(usize, usize, usize),
    /// Read some memory, then write some.
    ReadWrite(usize, usize, usize),
}

impl OpRd {
    #[inline(always)]
    pub fn set_tid(&mut self, tid: usize) {
        match self {
            OpRd::ReadOnly(ref mut a, _b, _c) => *a = tid,
        };
    }
}

impl OpWr {
    #[inline(always)]
    pub fn set_tid(&mut self, tid: usize) {
        match self {
            OpWr::WriteOnly(ref mut a, _b, _c) => *a = tid,
            OpWr::ReadWrite(ref mut a, _b, _c) => *a = tid,
        };
    }
}

#[derive(Debug, Clone)]
pub struct AbstractDataStructure {
    /// Total cache-lines
    n: usize,
    /// Amount of reads for cold-reads.
    cold_reads: usize,
    /// Amount of writes for cold-writes.
    cold_writes: usize,
    /// Amount of hot cache-lines read.
    hot_reads: usize,
    /// Amount of hot writes to cache-lines
    hot_writes: usize,
    /// Backing memory
    storage: Vec<CachePadded<usize>>,
}

impl Default for AbstractDataStructure {
    fn default() -> Self {
        AbstractDataStructure::new(200_000, 20, 5, 2, 1)
    }
}

impl AbstractDataStructure {
    fn new(
        n: usize,
        cold_reads: usize,
        cold_writes: usize,
        hot_reads: usize,
        hot_writes: usize,
    ) -> AbstractDataStructure {
        debug_assert!(hot_reads + cold_writes < n);
        debug_assert!(hot_reads + cold_reads < n);
        debug_assert!(hot_writes < hot_reads);

        // Maximum buffer space (within a data-structure).
        const MAX_BUFFER_SIZE: usize = 400_000;
        debug_assert!(n < MAX_BUFFER_SIZE);

        let mut storage = Vec::with_capacity(n);
        for i in 0..n {
            storage.push(CachePadded::from(i));
        }

        AbstractDataStructure {
            n,
            cold_reads,
            cold_writes,
            hot_reads,
            hot_writes,
            storage,
        }
    }

    pub fn read(&self, tid: usize, rnd1: usize, rnd2: usize) -> usize {
        let mut sum = 0;

        // Hot cache-lines (reads sequential)
        let begin = rnd2;
        let end = begin + self.hot_writes;
        for i in begin..end {
            let index = i % self.hot_reads;
            sum += *self.storage[index];
        }

        // Cold cache-lines (random stride reads)
        let mut begin = rnd1 * tid;
        for _i in 0..self.cold_reads {
            let index = begin % (self.n - self.hot_reads) + self.hot_reads;
            begin += rnd2;
            sum += *self.storage[index];
        }

        sum
    }

    pub fn write(&mut self, tid: usize, rnd1: usize, rnd2: usize) -> usize {
        // Hot cache-lines (updates sequential)
        let begin = rnd2;
        let end = begin + self.hot_writes;
        for i in begin..end {
            let index = i % self.hot_reads;
            self.storage[index] = CachePadded::new(tid);
        }

        // Cold cache-lines (random stride updates)
        let mut begin = rnd1 * tid;
        for _i in 0..self.cold_writes {
            let index = begin % (self.n - self.hot_reads) + self.hot_reads;
            begin += rnd2;
            self.storage[index] = CachePadded::new(tid);
        }

        0
    }

    pub fn read_write(&mut self, tid: usize, rnd1: usize, rnd2: usize) -> usize {
        // Hot cache-lines (sequential updates)
        let begin = rnd2;
        let end = begin + self.hot_writes;
        for i in begin..end {
            let index = i % self.hot_reads;
            self.storage[index] = CachePadded::new(*self.storage[index] + 1);
        }

        // Cold cache-lines (random stride updates)
        let mut sum = 0;
        let mut begin = rnd1 * tid;
        for _i in 0..self.cold_writes {
            let index = begin % (self.n - self.hot_reads) + self.hot_reads;
            begin += rnd2;
            sum += *self.storage[index];
            self.storage[index] = CachePadded::new(*self.storage[index] + 1);
        }

        sum
    }
}

impl Dispatch for AbstractDataStructure {
    type ReadOperation = OpRd;
    type WriteOperation = OpWr;
    type Response = usize;
    type ResponseError = ();

    fn dispatch(&self, op: Self::ReadOperation) -> Result<Self::Response, Self::ResponseError> {
        match op {
            OpRd::ReadOnly(a, b, c) => return Ok(self.read(a, b, c)),
        }
    }

    /// Implements how we execute operation from the log against abstract DS
    fn dispatch_mut(
        &mut self,
        op: Self::WriteOperation,
    ) -> Result<Self::Response, Self::ResponseError> {
        match op {
            OpWr::WriteOnly(a, b, c) => return Ok(self.write(a, b, c)),
            OpWr::ReadWrite(a, b, c) => return Ok(self.read_write(a, b, c)),
        }
    }
}

/// Generate a random sequence of operations that we'll perform.
///
/// Flag determines which types of operation we allow on the data-structure.
/// The split is approximately equal among the operations we allow.
fn generate_operation(
    rng: &mut rand::rngs::SmallRng,
    tid: usize,
    readonly: bool,
    writeonly: bool,
    readwrite: bool,
) -> Operation<OpRd, OpWr> {
    let op: usize = rng.gen::<usize>();
    match (readonly, writeonly, readwrite) {
        (true, true, true) => match op % 3 {
            0 => Operation::ReadOperation(OpRd::ReadOnly(tid, rng.gen(), rng.gen())),
            1 => Operation::WriteOperation(OpWr::WriteOnly(tid, rng.gen(), rng.gen())),
            2 => Operation::WriteOperation(OpWr::ReadWrite(tid, rng.gen(), rng.gen())),
            _ => unreachable!(),
        },
        (false, true, true) => match op % 2 {
            0 => Operation::WriteOperation(OpWr::WriteOnly(tid, rng.gen(), rng.gen())),
            1 => Operation::WriteOperation(OpWr::ReadWrite(tid, rng.gen(), rng.gen())),
            _ => unreachable!(),
        },
        (true, true, false) => match op % 2 {
            0 => Operation::ReadOperation(OpRd::ReadOnly(tid, rng.gen(), rng.gen())),
            1 => Operation::WriteOperation(OpWr::WriteOnly(tid, rng.gen(), rng.gen())),
            _ => unreachable!(),
        },
        (true, false, true) => match op % 2 {
            0 => Operation::ReadOperation(OpRd::ReadOnly(tid, rng.gen(), rng.gen())),
            1 => Operation::WriteOperation(OpWr::ReadWrite(tid, rng.gen(), rng.gen())),
            _ => unreachable!(),
        },
        (true, false, false) => Operation::ReadOperation(OpRd::ReadOnly(tid, rng.gen(), rng.gen())),
        (false, true, false) => {
            Operation::WriteOperation(OpWr::WriteOnly(tid, rng.gen(), rng.gen()))
        }
        (false, false, true) => {
            Operation::WriteOperation(OpWr::ReadWrite(tid, rng.gen(), rng.gen()))
        }
        (false, false, false) => panic!("no operations selected"),
    }
}

/// Compare a synthetic benchmark against a single-threaded implementation.
fn synthetic_single_threaded(c: &mut TestHarness) {
    // Size of the log.
    const LOG_SIZE_BYTES: usize = 2 * 1024 * 1024;

    mkbench::baseline_comparison::<AbstractDataStructure>(
        c,
        "synthetic",
        LOG_SIZE_BYTES,
        &mut |rng| generate_operation(rng, 1, false, false, true),
    );
}

/// Compare scale-out behaviour of synthetic data-structure.
fn synthetic_scale_out(c: &mut TestHarness) {
    mkbench::ScaleBenchBuilder::new()
        .machine_defaults()
        .configure::<AbstractDataStructure>(
            c,
            "synthetic-scaleout",
            |cid, rid, _log, replica, _batch_size, rng| match generate_operation(
                rng,
                cid as usize,
                false,
                false,
                true,
            ) {
                Operation::ReadOperation(mut o) => {
                    o.set_tid(cid as usize);
                    replica.execute_ro(o, rid).unwrap();
                }
                Operation::WriteOperation(mut o) => {
                    o.set_tid(cid as usize);
                    replica.execute(o, rid).unwrap();
                }
            },
        );
}

fn main() {
    let _r = env_logger::try_init();
    let mut harness = Default::default();

    synthetic_single_threaded(&mut harness);
    synthetic_scale_out(&mut harness);
}
