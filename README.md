# Schemamama

A lightweight database schema migration system. *Sche·ma·ma·ma*. See [Schemamama's adapters](#adapters) for full usage examples for your chosen database technology.

Schemamama aims to work on all Rust releases: stable, beta, and nightly!

## Installation

If you're using Cargo, just add Schemamama to your `Cargo.toml`:

```toml
[dependencies]
schemamama = "*"
```

## Adapters

Schemamama offers a modular design that allows for interfacing with any database technology. Here's a list of known adapters:

* [PostgreSQL](https://github.com/SkylerLipthay/schemamama_postgres)
* [SQLite3](https://github.com/cmsd2/schemamama_rusqlite)

## To-do

* Command line interface
