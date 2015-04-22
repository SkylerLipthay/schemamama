#![feature(collections)]

#[macro_use]
extern crate log;

use std::collections::{BTreeMap, BTreeSet, Bound};

/// The version type alias used to uniquely reference migrations.
pub type Version = i64;

/// All migrations will implement this trait, and a migration trait specific to the chosen adapter.
/// This trait defines the metadata for tracking migration sequence and for human reference.
pub trait Migration {
    /// An ordered (but not necessarily sequential), unique identifier for this migration.
    /// Registered migrations will be applied in ascending order by version.
    fn version(&self) -> Version;

    /// A message describing the effects of this migration.
    fn description(&self) -> &'static str;
}

/// Efficiently implement the `Migration` trait for a given type.
///
/// ## Example
///
/// ```rust
/// # #[macro_use]
/// # extern crate schemamama;
/// struct MyMigration;
/// migration!(MyMigration, 100, "create some lovely database tables");
///
/// # fn main() {
/// use schemamama::Migration;
/// let m = MyMigration;
/// assert_eq!(m.version(), 100);
/// assert_eq!(m.description(), "create some lovely database tables");
/// # }
/// ```
#[macro_export]
macro_rules! migration {
    ($ty:ident, $version:expr, $description:expr) => {
        impl $crate::Migration for $ty {
            fn version(&self) -> $crate::Version { $version }
            fn description(&self) -> &'static str { $description }
        }
    }
}

/// Use this trait to connect the migrator to your chosen database technology.
pub trait Adapter {
    /// An alias to a specific trait that extends `Migration`. Typically, the aforementioned trait
    /// will declare functions that the adapter will use to migrate upwards and downwards.
    type MigrationType: Migration + ?Sized;

    /// Returns the latest migration version, or `None` if no migrations have been recorded. Can
    /// panic if necessary.
    fn current_version(&self) -> Option<Version>;

    /// Returns a set of the versions of all of the currently applied migrations. Can panic if
    /// necessary.
    fn migrated_versions(&self) -> BTreeSet<Version>;

    /// Applies the specified migration. Can panic if necessary.
    fn apply_migration(&self, migration: &Self::MigrationType);

    /// Reverts the specified migration. Can panic if necessary.
    fn revert_migration(&self, migration: &Self::MigrationType);
}

/// Maintains an ordered collection of migrations to utilize.
pub struct Migrator<T: Adapter> {
    adapter: T,
    migrations: BTreeMap<Version, Box<T::MigrationType>>
}

impl<T: Adapter> Migrator<T> {
    /// Create a migrator with a given adapter.
    pub fn new(adapter: T) -> Migrator<T> {
        Migrator { adapter: adapter, migrations: BTreeMap::new() }
    }

    /// Get a reference to the adapter.
    pub fn adapter(&self) -> &T {
        &self.adapter
    }

    /// Register a migration. If a migration with the same version is already registered, a warning
    /// is logged and the registration fails.
    pub fn register(&mut self, migration: Box<T::MigrationType>) {
        let version = migration.version();
        if self.version_registered(version) {
            warn!("Migration with version {:?} is already registered", version);
        } else {
            self.migrations.insert(version, migration);
        }
    }

    /// Returns true is a migration with the provided version has been registered.
    pub fn version_registered(&self, version: Version) -> bool {
        self.migrations.contains_key(&version)
    }

    /// Returns the lowest version of the registered migrations, or `None` if no migrations have
    /// been registered.
    pub fn first_version(&self) -> Option<Version> {
        self.migrations.keys().next().map(|v| *v)
    }

    /// Returns the highest version of the registered migrations, or `None` if no migrations have
    /// been registered.
    pub fn last_version(&self) -> Option<Version> {
        self.migrations.keys().last().map(|v| *v)
    }

    /// Returns the latest migration version, or `None` if no migrations have been recorded.
    ///
    /// ## Panics
    ///
    /// Panics if there is an underlying problem retrieving the current version from the adapter.
    pub fn current_version(&self) -> Option<Version> {
        self.adapter.current_version()
    }

    /// Returns a set of the versions of all of the currently applied migrations.
    ///
    /// ## Panics
    ///
    /// Panics if there is an underlying problem retrieving the current versions from the adapter.
    pub fn migrated_versions(&self) -> BTreeSet<Version> {
        self.adapter.migrated_versions()
    }

    /// Rollback to the specified version (exclusive), or rollback to the state before any
    /// registered migrations were applied if `None` is specified.
    ///
    /// ## Panics
    ///
    /// Panics if there is an underlying problem reverting any of the matched migrations.
    pub fn down(&self, to: Option<Version>) {
        let migrated_versions = self.migrated_versions();
        let current_version = self.current_version();
        let source = match current_version {
            Some(ref version) => Bound::Included(version),
            None => return
        };
        let destination = match to {
            Some(ref version) => Bound::Excluded(version),
            None => Bound::Unbounded
        };
        for (version, migration) in self.migrations.range(destination, source).rev() {
            if !migrated_versions.contains(version) {
                info!("Skipping migration {:?}: {}", version, migration.description());
                continue;
            }
            info!("Reverting migration {:?}: {}", version, migration.description());
            self.adapter.revert_migration(migration);
        }
    }

    /// Migrate to the specified version (inclusive).
    ///
    /// ## Panics
    ///
    /// Panics if there is an underlying problem applying any of the matched migrations.
    pub fn up(&self, to: Version) {
        let migrated_versions = self.migrated_versions();
        let destination = Bound::Included(&to);
        for (version, migration) in self.migrations.range(Bound::Unbounded, destination) {
            if migrated_versions.contains(version) {
                info!("Skipping migration {:?}: {}", version, migration.description());
                continue;
            }
            info!("Applying migration {:?}: {}", version, migration.description());
            self.adapter.apply_migration(migration);
        }
    }
}
