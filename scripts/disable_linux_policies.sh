#!/bin/bash
#
# Usage: bash scripts/disable_linux_policies.sh
#
set -ex

# Disable all the Linux policies that are up to no good
sudo sh -c "echo 0 > /proc/sys/kernel/numa_balancing"
sudo sh -c "echo 0 > /sys/kernel/mm/ksm/run"
sudo sh -c "echo 0 > /sys/kernel/mm/ksm/merge_across_nodes"
sudo sh -c "echo never > /sys/kernel/mm/transparent_hugepage/enabled"

# Disable hyperthreading
sudo sh -c "echo off > /sys/devices/system/cpu/smt/control"
