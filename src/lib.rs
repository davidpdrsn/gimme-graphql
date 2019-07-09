#![feature(proc_macro_hygiene, decl_macro)]
#![forbid(unknown_lints)]
// #![deny(unused_imports, dead_code, unused_variables)]

pub mod rocket_adapter;

use diesel::r2d2::ConnectionManager;
use juniper::GraphQLType;

pub trait GraphqlApp {
    type DatabaseConnection: 'static + diesel::Connection;
    type WebFramework: WebFramework<
        Self::DatabaseConnection,
        Self::Query,
        Self::Mutation,
        Self::Context,
    >;
    type Query: GraphQLType<TypeInfo = (), Context = Self::Context>;
    type Mutation: GraphQLType<TypeInfo = (), Context = Self::Context>;
    type Context: 'static + juniper::Context;

    fn run() {
        dotenv::dotenv().ok();
        env_logger::init();

        let framework = Self::WebFramework::new();
        let database_connection_pool = Self::create_database_connection_pool();
        framework.run(database_connection_pool);
    }

    fn database_connection_pool_max_size() -> u32 {
        10
    }

    fn database_url() -> String {
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    }

    fn create_database_connection_pool() -> DbConPool<Self::DatabaseConnection> {
        let connection_manager =
            ConnectionManager::<Self::DatabaseConnection>::new(Self::database_url());

        r2d2::Pool::builder()
            .max_size(Self::database_connection_pool_max_size())
            .build(connection_manager)
            .expect("failed to create db connection pool")
    }
}

pub type DbConPool<Connection> = r2d2::Pool<ConnectionManager<Connection>>;
pub type DbCon<Connection> = r2d2::PooledConnection<ConnectionManager<Connection>>;

pub trait WebFramework<Connection, Query, Mutation, Context>
where
    Connection: 'static + diesel::Connection,
{
    fn new() -> Self;

    fn run(&self, database_connection_pool: DbConPool<Connection>);
}
