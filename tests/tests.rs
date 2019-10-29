#[macro_use]
extern crate schemamama;

use schemamama::{Adapter, Migration, Migrator, Version};
use std::cell::RefCell;
use std::collections::BTreeSet;

struct DummyAdapter {
    versions: RefCell<BTreeSet<Version>>
}

impl DummyAdapter {
    pub fn new() -> DummyAdapter {
        DummyAdapter { versions: RefCell::new(BTreeSet::new()) }
    }

    pub fn is_migrated(&self, version: Version) -> bool {
        self.versions.borrow().contains(&version)
    }
}

impl Adapter for DummyAdapter {
    type MigrationType = Migration;
    type Error = ();

    fn current_version(&mut self) -> Result<Option<Version>, ()> {
        Ok(self.versions.borrow().iter().last().map(|v| *v))
    }

    fn migrated_versions(&mut self) -> Result<BTreeSet<Version>, ()> {
        Ok(self.versions.borrow().iter().cloned().collect())
    }

    fn apply_migration(&mut self, migration: &Migration) -> Result<(), ()> {
        self.versions.borrow_mut().insert(migration.version());
        Ok(())
    }

    fn revert_migration(&mut self, migration: &Migration) -> Result<(), ()> {
        self.versions.borrow_mut().remove(&migration.version());
        Ok(())
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
    let mut versions = BTreeSet::new();
    versions.insert(10);
    versions.insert(20);
    assert_eq!(migrator.registered_versions(), versions);
}

#[test]
fn test_version_registered() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    assert_eq!(migrator.version_registered(10), false);
    migrator.register(Box::new(FirstMigration));
    assert_eq!(migrator.version_registered(10), true);
}

#[test]
fn test_migrate() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    migrator.register(Box::new(FirstMigration));
    migrator.register(Box::new(SecondMigration));
    assert_eq!(migrator.current_version().unwrap(), None);
    migrator.up(Some(20)).unwrap();
    assert_eq!(migrator.current_version().unwrap(), Some(20));
    migrator.down(Some(10)).unwrap();
    assert_eq!(migrator.current_version().unwrap(), Some(10));
    migrator.down(None).unwrap();
    assert_eq!(migrator.current_version().unwrap(), None);
    migrator.up(None).unwrap();
    assert_eq!(migrator.current_version().unwrap(), Some(20));
}

#[test]
fn test_retroactive_migrations() {
    let mut migrator = Migrator::new(DummyAdapter::new());
    migrator.register(Box::new(SecondMigration));
    migrator.up(Some(20)).unwrap();
    assert_eq!(migrator.current_version().unwrap(), Some(20));
    assert!(migrator.adapter().is_migrated(20));
    assert!(!migrator.adapter().is_migrated(10));
    migrator.register(Box::new(FirstMigration));
    migrator.up(Some(20)).unwrap();
    assert_eq!(migrator.current_version().unwrap(), Some(20));
    assert!(migrator.adapter().is_migrated(20));
    assert!(migrator.adapter().is_migrated(10));
}
