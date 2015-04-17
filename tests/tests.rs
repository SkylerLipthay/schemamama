#[macro_use]
extern crate schemamama;

use schemamama::{Adapter, Migration, Migrator, Version};
use std::cell::RefCell;

struct DummyAdapter {
    versions: RefCell<Vec<Version>>
}

impl DummyAdapter {
    pub fn new() -> DummyAdapter {
        DummyAdapter { versions: RefCell::new(Vec::new()) }
    }
}

impl Adapter for DummyAdapter {
    type MigrationType = Migration;

    fn current_version(&self) -> Option<Version> {
        self.versions.borrow_mut().iter().last().map(|v| *v)
    }

    fn apply_migration(&self, migration: &Migration) {
        self.versions.borrow_mut().push(migration.version());
    }

    fn revert_migration(&self, migration: &Migration) {
        let mut versions = self.versions.borrow_mut();
        versions.iter().position(|&v| v == migration.version()).map(|i| versions.remove(i));
    }
}

struct FirstMigration;
migration!(FirstMigration, 10, "first migration");
struct SecondMigration;
migration!(SecondMigration, 20, "second migration");

#[test]
fn test_registration() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    assert_eq!(migrator.first_version(), None);
    assert_eq!(migrator.last_version(), None);
    migrator.register(Box::new(SecondMigration));
    migrator.register(Box::new(FirstMigration));
    assert_eq!(migrator.first_version(), Some(10));
    assert_eq!(migrator.last_version(), Some(20));
}

#[test]
fn test_has_version() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    assert_eq!(migrator.has_version(10), false);
    migrator.register(Box::new(FirstMigration));
    assert_eq!(migrator.has_version(10), true);
}

#[test]
fn test_migrate() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    migrator.register(Box::new(FirstMigration));
    migrator.register(Box::new(SecondMigration));
    assert_eq!(migrator.current_version(), None);
    migrator.up(20);
    assert_eq!(migrator.current_version(), Some(20));
    migrator.down(Some(10));
    assert_eq!(migrator.current_version(), Some(10));
    migrator.down(None);
    assert_eq!(migrator.current_version(), None);
}
