extern crate schemamama;
extern crate postgres;

use schemamama::{Migration, Migrator};

use postgres::{Connection, SslMode};

fn make_database_connection() -> Connection {
    let connection = Connection::connect("postgres://postgres@localhost", &SslMode::None).unwrap();
    connection.execute("SET search_path TO pg_temp;", &[]).unwrap();
    connection
}

fn current_schema_name(connection: &Connection) -> String {
    connection.prepare("SELECT CURRENT_SCHEMA();").unwrap().query(&[]).unwrap().iter().next().
        map(|r| r.get(0)).unwrap()
}

#[test]
fn test_registration() {
    let connection = make_database_connection();
    let mut migrator = Migrator::new(&connection);

    assert_eq!(migrator.earliest_registered_version(), None);
    assert_eq!(migrator.latest_registered_version(), None);

    migrator.register(Box::new(ThirdMigration));
    migrator.register(Box::new(SecondMigration));

    assert_eq!(migrator.earliest_registered_version(), Some(2));
    assert_eq!(migrator.latest_registered_version(), Some(3));

    migrator.register(Box::new(FirstMigration));

    assert_eq!(migrator.earliest_registered_version(), Some(1));
}

#[test]
fn test_setup() {
    let connection = make_database_connection();
    let schema_name = current_schema_name(&connection);
    let migrator = Migrator::new(&connection);
    let query = "SELECT * FROM pg_catalog.pg_tables WHERE schemaname = $1 AND \
                 tablename = 'schemamama';";

    for _ in 0..2 {
        migrator.setup_schema();
        assert_eq!(connection.execute(query, &[&schema_name]).unwrap(), 1);
    }
}

#[test]
fn test_migration_count() {
    let connection = make_database_connection();
    let mut migrator = Migrator::new(&connection);
    migrator.register(Box::new(FirstMigration));
    migrator.register(Box::new(SecondMigration));
    migrator.register(Box::new(ThirdMigration));

    migrator.setup_schema();
    migrator.up(std::i64::MAX);
    assert_eq!(migrator.latest_schema_version(), Some(3));

    migrator.down(2);
    assert_eq!(migrator.latest_schema_version(), Some(2));
}

#[test]
fn test_migration_up_and_down() {
    let connection = make_database_connection();
    let schema_name = current_schema_name(&connection);
    let mut migrator = Migrator::new(&connection);
    migrator.register(Box::new(FirstMigration));

    migrator.setup_schema();
    migrator.up(1);
    let query = "SELECT * FROM pg_catalog.pg_tables WHERE schemaname = $1 AND \
                 tablename = 'first';";
    assert_eq!(connection.execute(query, &[&schema_name]).unwrap(), 1);

    migrator.down(0);
    assert_eq!(connection.execute(query, &[&schema_name]).unwrap(), 0);
}

struct FirstMigration;

impl Migration for FirstMigration {
    fn version(&self) -> i64 { 1 }

    fn up(&self, transaction: &postgres::Transaction) {
        transaction.execute("CREATE TABLE first (id BIGINT PRIMARY KEY);", &[]).unwrap();
    }

    fn down(&self, transaction: &postgres::Transaction) {
        transaction.execute("DROP TABLE first;", &[]).unwrap();
    }
}

struct SecondMigration;

impl Migration for SecondMigration {
    fn version(&self) -> i64 { 2 }
}

struct ThirdMigration;

impl Migration for ThirdMigration {
    fn version(&self) -> i64 { 3 }
}
