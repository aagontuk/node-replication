// Copyright © 2019-2022 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Utility functions to do multi-threaded benchmarking.
//!
//! Mostly these definitions are leftovers from the time when we used criterion
//! and stayed for compatibility with the old benchmarking code.

#![allow(unused)]
use std::fmt::Display;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Throughput(pub(crate) u64);

pub struct Bencher {
    /// How long do we measure (this is fixed by the runner)
    pub(crate) measurement_time: Duration,
    /// How many operations did we perform (what we measured as throughput).
    iterations: usize,
}

impl Bencher {
    fn new(duration: Duration) -> Bencher {
        Bencher {
            measurement_time: duration,
            iterations: 0,
        }
    }

    pub(crate) fn iter<R>(&mut self, mut routine: R)
    where
        R: FnMut() -> usize,
    {
        self.iterations += routine();
    }

    pub(crate) fn iter_custom<R>(&mut self, mut routine: R)
    where
        R: FnMut(Duration) -> usize,
    {
        self.iterations = routine(self.measurement_time);
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct BenchmarkId {
    pub(crate) function_name: Option<String>,
    pub(crate) parameter: Option<String>,
}

impl BenchmarkId {
    pub(crate) fn new<S: Into<String>, P: Display>(function_name: S, parameter: P) -> BenchmarkId {
        BenchmarkId {
            function_name: Some(function_name.into()),
            parameter: Some(format!("{}", parameter)),
        }
    }
}

impl From<String> for BenchmarkId {
    fn from(string: String) -> BenchmarkId {
        BenchmarkId {
            function_name: Some(string),
            parameter: None,
        }
    }
}

impl From<&str> for BenchmarkId {
    fn from(string: &str) -> BenchmarkId {
        BenchmarkId {
            function_name: Some(String::from(string)),
            parameter: None,
        }
    }
}

pub(crate) struct BenchmarkGroup {
    pub(crate) group_name: String,
    pub(crate) duration: Duration,
}

impl BenchmarkGroup {
    /// Set the input size for this benchmark group. Used for reporting the
    /// duration.
    pub(crate) fn duration(&mut self, duration: Duration) -> &mut Self {
        self.duration = duration;
        self
    }

    /// Benchmark the given parameterless function inside this benchmark group.
    pub(crate) fn bench_function<ID: Into<BenchmarkId>, F>(&mut self, id: ID, mut f: F) -> &mut Self
    where
        F: FnMut(&mut Bencher),
    {
        let bid = id.into();
        println!(
            "Run {}/{}:",
            bid.function_name.unwrap_or(String::from("unknown")),
            bid.parameter.unwrap_or(String::from("unknown"))
        );

        let mut bencher = Bencher::new(self.duration);
        f(&mut bencher);

        self
    }

    pub fn bench_with_input<ID: Into<BenchmarkId>, F, I>(
        &mut self,
        id: ID,
        input: &I,
        mut f: F,
    ) -> &mut Self
    where
        F: FnMut(&mut Bencher, &I),
    {
        let bid = id.into();
        print!(
            "Run {}:",
            bid.function_name.unwrap_or(String::from("unknown")),
        );
        println!("/{}", bid.parameter.unwrap_or(String::from("")));

        let mut bencher = Bencher::new(self.duration);
        f(&mut bencher, &input);

        self
    }

    pub fn finish(self) {}
}

pub struct TestHarness {
    pub(crate) duration: Duration,
}

impl TestHarness {
    pub(crate) fn benchmark_group<S: Into<String>>(&mut self, group_name: S) -> BenchmarkGroup {
        BenchmarkGroup {
            group_name: group_name.into(),
            duration: self.duration,
        }
    }
}

impl TestHarness {
    pub fn new(d: Duration) -> Self {
        if cfg!(feature = "smokebench") {
            log::warn!("smokebench enabled, force execution to 500 ms");
            let d = Duration::from_millis(500);
            TestHarness { duration: d }
        } else {
            TestHarness { duration: d }
        }
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        if cfg!(feature = "smokebench") {
            TestHarness::new(Duration::from_millis(500))
        } else {
            TestHarness::new(Duration::from_secs(5))
        }
    }
}

pub fn mean(data: &[usize]) -> Option<f64> {
    let sum = data.iter().sum::<usize>() as f64;
    let count = data.len();

    match count {
        positive if positive > 0 => Some(sum / count as f64),
        _ => None,
    }
}

pub fn std_deviation(data: &[usize]) -> Option<f64> {
    match (mean(data), data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let variance = data
                .iter()
                .map(|value| {
                    let diff = data_mean - (*value as f64);

                    diff * diff
                })
                .sum::<f64>()
                / count as f64;

            Some(variance.sqrt())
        }
        _ => None,
    }
}
