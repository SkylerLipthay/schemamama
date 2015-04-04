#![feature(collections)]

extern crate postgres;

use std::collections::{Bound, BTreeMap};

pub trait Migration {
    /// An ordered, unique identifier for this migration. Registered migrations will be run in
    /// ascending order by version.
    fn version(&self) -> i64;

    /// Called when this migration is to be executed. This function has an empty body by default,
    /// so its implementation is optional.
    fn up(&self, _transaction: &postgres::Transaction) { }

    /// Called when this migration is to be reversed. This function has an empty body by default,
    /// so its implementation is optional.
    fn down(&self, _transaction: &postgres::Transaction) { }
}

pub struct Migrator<'a> {
    connection: &'a postgres::Connection,
    migrations: BTreeMap<i64, Box<Migration>>
}

impl<'a> Migrator<'a> {
    /// Create a new migrator tied to a PostgreSQL connection.
    pub fn new(connection: &'a postgres::Connection) -> Migrator {
        Migrator {
            connection: connection,
            migrations: BTreeMap::new()
        }
    }

    /// Register a bidirectional migration.
    ///
    /// ## Panics
    ///
    /// Panics if a migration with the same version has already been registered.
    pub fn register(&mut self, migration: Box<Migration>) {
        let version = migration.version();
        if self.migrations.contains_key(&version) {
            panic!("migration with version {} is already registered", version);
        }
        self.migrations.insert(version, migration);
    }

    /// Reverse the schema to how it was before the most recent registered migration was run.
    ///
    /// ## Panics
    ///
    /// Panics if any registered migrations panic during downwards migration. Panics if the
    /// Schemamama tables do not exist (call `setup_schema` prior to this).
    pub fn down(&self, to: i64) {
        let latest_version = self.latest_schema_version();
        let current = match latest_version {
            Some(ref version) => Bound::Included(version),
            None => return
        };
        let destination = Bound::Excluded(&to);
        let transaction = self.connection.transaction().unwrap();
        for (_, ref migration) in self.migrations.range(destination, current).rev() {
            migration.down(&transaction);
            self.delete_schema_version(migration.version());
        }
        transaction.commit().unwrap();
    }

    /// Migrate the schema to the most recent registered migration.
    ///
    /// ## Panics
    ///
    /// Panics if any registered migrations panic during upwards migration. Panics if the
    /// Schemamama tables do not exist (call `setup_schema` prior to this).
    pub fn up(&self, to: i64) {
        let latest_version = self.latest_schema_version();
        let current = match latest_version {
            Some(ref version) => Bound::Excluded(version),
            None => Bound::Unbounded
        };
        let destination = Bound::Included(&to);
        let transaction = self.connection.transaction().unwrap();
        for (_, ref migration) in self.migrations.range(current, destination) {
            migration.up(&transaction);
            self.record_schema_version(migration.version());
        }
        transaction.commit().unwrap();
    }

    /// Create the tables Schemamama requires to keep track of schema state. If the tables already
    /// exist, this function has no operation.
    ///
    /// ## Panics
    ///
    /// Panics if the connection fails to create, read, or update Schemamama-specific tables.
    pub fn setup_schema(&self) {
        let query = "CREATE TABLE IF NOT EXISTS schemamama (version BIGINT PRIMARY KEY);";
        self.connection.execute(query, &[]).unwrap();
    }

    /// Returns the latest migration version, or `None` if no migrations have been recorded.
    ///
    /// ## Panics
    ///
    /// Panics if the Schemamama tables do not exist (call `setup_schema` prior to this).
    pub fn latest_schema_version(&self) -> Option<i64> {
        let query = "SELECT version FROM schemamama ORDER BY version DESC LIMIT 1;";
        let statement = self.connection.prepare(query).unwrap();
        statement.query(&[]).unwrap().iter().next().map(|r| r.get(0))
    }

    /// Returns the lowest version of the registered migrations, or `None` if no migrations have
    /// been registered.
    pub fn earliest_registered_version(&self) -> Option<i64> {
        self.migrations.keys().next().map(|v| *v)
    }

    /// Returns the highest version of the registered migrations, or `None` if no migrations have
    /// been registered.
    pub fn latest_registered_version(&self) -> Option<i64> {
        self.migrations.keys().rev().next().map(|v| *v)
    }

    // Panics if the Schemamama tables do not exist (call `setup_schema` prior to this).
    fn delete_schema_version(&self, version: i64) {
        let query = "DELETE FROM schemamama WHERE version = $1;";
        self.connection.execute(query, &[&version]).unwrap();
    }

    // Panics if the Schemamama tables do not exist (call `setup_schema` prior to this).
    fn record_schema_version(&self, version: i64) {
        let query = "INSERT INTO schemamama (version) VALUES ($1);";
        self.connection.execute(query, &[&version]).unwrap();
    }
}
