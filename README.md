# Schemamama

A lightweight database schema migration system. Supports only PostgreSQL for
now. Depends on `rust-postgres`. *Sche·ma·ma·ma*.

## Testing

To run `cargo test`, you must have PostgreSQL running locally with a user role
named `postgres` with login access to a database named `postgres`. All tests
will work in the `pg_temp` schema, so the database will not be modified.
