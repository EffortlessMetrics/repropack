set shell := ["bash", "-cu"]

default:
    @just --list

ci-fast:
    cargo xtask ci-fast

ci-full:
    cargo xtask ci-full

smoke:
    cargo xtask smoke

scenario-index:
    cargo xtask scenario-index

mutants:
    cargo xtask mutants

docs-check:
    cargo xtask docs-check

release-check:
    cargo xtask release-check
