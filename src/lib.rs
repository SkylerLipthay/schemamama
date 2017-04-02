#[macro_use]
extern crate log;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

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

/// A migration's direction.
#[derive(Debug)]
pub enum Direction {
    Down,
    Up,
}

/// An all-encompassing error type that can be returned during interaction with the migrator
/// adapter.
#[derive(Debug)]
pub enum Error<E> {
    /// A generic error that occurred while interacting with the adapter.
    Adapter(E),
    /// An error that arose from the adapter specifically during a migration's execution.
    Migration {
        /// The version of the migration that failed.
        version: Version,
        /// The description of the migration that failed.
        description: &'static str,
        /// The direction in which the failed migration was ran.
        direction: Direction,
        /// The underlying error from the adapter.
        error: E,
    }
}

impl<E: std::error::Error> std::error::Error for Error<E> {
    fn description(&self) -> &str {
        match *self {
            Error::Adapter(ref err) => err.description(),
            Error::Migration{version: _, description: _, direction: _, ref error} => error.description(),
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::Adapter(ref err) => Some(err),
            Error::Migration{version: _, description: _, direction: _, ref error} => Some(error),
        }
    }
}

impl<E: std::error::Error> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match *self {
            Error::Adapter(ref err) => write!(f, "Adataper error: {}", err),
            Error::Migration{version: _, ref description, direction: _, ref error} => write!(f, "Error running migration {}, error: {}", description, error),
        }
    }
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

    /// An adapter-specific error type that can be returned from any of this trait's methods.
    type Error;

    /// Returns the latest migration version, or `None` if no migrations have been recorded.
    fn current_version(&self) -> Result<Option<Version>, Self::Error>;

    /// Returns a set of the versions of all of the currently applied migrations.
    fn migrated_versions(&self) -> Result<BTreeSet<Version>, Self::Error>;

    /// Applies the specified migration.
    fn apply_migration(&self, migration: &Self::MigrationType) -> Result<(), Self::Error>;

    /// Reverts the specified migration.
    fn revert_migration(&self, migration: &Self::MigrationType) -> Result<(), Self::Error>;
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

    /// Returns the set of all registered migration versions.
    pub fn registered_versions(&self) -> BTreeSet<Version> {
        self.migrations.keys().cloned().collect()
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
    pub fn current_version(&self) -> Result<Option<Version>, Error<T::Error>> {
        match self.adapter.current_version() {
            Ok(ver) => Ok(ver),
            Err(err) => Err(Error::Adapter(err)),
        }
    }

    /// Returns a set of the versions of all of the currently applied migrations.
    pub fn migrated_versions(&self) -> Result<BTreeSet<Version>, Error<T::Error>> {
        match self.adapter.migrated_versions() {
            Ok(vers) => Ok(vers),
            Err(err) => Err(Error::Adapter(err)),
        }
    }

    /// Rollback to the specified version (exclusive), or rollback to the state before any
    /// registered migrations were applied if `None` is specified.
    pub fn down(&self, to: Option<Version>) -> Result<(), Error<T::Error>> {
        let from = try!(self.current_version());
        if from.is_none() {
            return Ok(());
        }

        let migrated_versions = try!(self.migrated_versions());
        let targets = self.migrations.iter()
            // Rollback migrations from latest to oldest:
            .rev()
            // Rollback the current version, and all versions downwards until the specified version
            // (exclusive):
            .filter(|&(&v, _)| within_range(v, to, from))
            // Rollback only the migrations that are actually already migrated (in the case that
            // some intermediary migrations were never executed).
            .filter(|&(v, _)| migrated_versions.contains(v));

        for (&version, migration) in targets {
            info!("Reverting migration {:?}: {}", version, migration.description());
            if let Err(err) = self.adapter.revert_migration(migration) {
                return Err(Error::Migration {
                    version: version,
                    description: migration.description(),
                    direction: Direction::Down,
                    error: err,
                });
            }
        }

        Ok(())
    }

    /// Migrate to the specified version (inclusive).
    pub fn up(&self, to: Option<Version>) -> Result<(), Error<T::Error>> {
        let migrated_versions = try!(self.migrated_versions());
        let targets = self.migrations.iter()
            // Execute all versions upwards until the specified version (inclusive):
            .filter(|&(&v, _)| within_range(v, None, to))
            // Execute only the migrations that are actually not already migrated (in the case that
            // some intermediary migrations were previously executed).
            .filter(|&(v, _)| !migrated_versions.contains(v));

        for (&version, migration) in targets {
            info!("Applying migration {:?}: {}", version, migration.description());
            if let Err(err) = self.adapter.apply_migration(migration) {
                return Err(Error::Migration {
                    version: version,
                    description: migration.description(),
                    direction: Direction::Up,
                    error: err,
                });
            }
        }

        Ok(())
    }
}

// Tests whether a `Version` is within a range defined by the exclusive `low` and the inclusive
// `high` bounds.
fn within_range(version: Version, low: Option<Version>, high: Option<Version>) -> bool {
    match (low, high) {
        (None, None) => true,
        (Some(low), None) => version > low,
        (None, Some(high)) => version <= high,
        (Some(low), Some(high)) => version > low && version <= high,
    }
}

#[test]
fn test_within_range() {
    // no lower or upper bound
    assert!(within_range(0, None, None));
    assert!(within_range(42, None, None));
    assert!(within_range(100000, None, None));

    // both lower and upper bounds
    assert!(!within_range(1, Some(2), Some(5)));
    assert!(!within_range(2, Some(2), Some(5)));
    assert!(within_range(3, Some(2), Some(5)));
    assert!(within_range(5, Some(2), Some(5)));
    assert!(!within_range(6, Some(2), Some(5)));

    // lower bound only
    assert!(!within_range(0, Some(5), None));
    assert!(!within_range(4, Some(5), None));
    assert!(!within_range(5, Some(5), None));
    assert!(within_range(6, Some(5), None));
    assert!(within_range(60, Some(5), None));

    // upper bound only
    assert!(within_range(0, None, Some(5)));
    assert!(within_range(5, None, Some(5)));
    assert!(!within_range(6, None, Some(5)));
}
