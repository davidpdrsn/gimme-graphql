#![feature(proc_macro_hygiene, decl_macro)]
#![forbid(unknown_lints)]
// #![deny(unused_imports, dead_code, unused_variables)]

pub mod rocket_adapter;

pub use diesel::r2d2::ConnectionManager;
pub use r2d2::{Pool, PooledConnection};

use juniper::GraphQLType;

pub fn run_graphql_app<App: GraphqlApp>(app: App) {
    dotenv::dotenv().ok();
    env_logger::init();

    let config = WebFrameworkConfig {
        database_connection_pool: create_database_connection_pool(&app),
        graphql_path: app.graphql_path(),
        mount_graphiql_at: app.mount_graphiql_at(),
        mount_graphql_at: app.mount_graphql_at(),
    };

    App::Adapter::new().run(app, config);
}

fn create_database_connection_pool<App: GraphqlApp>(
    app: &App,
) -> Pool<ConnectionManager<App::Connection>> {
    let connection_manager = ConnectionManager::<App::Connection>::new(app.database_url());

    r2d2::Pool::builder()
        .max_size(app.database_connection_pool_max_size())
        .build(connection_manager)
        .expect("failed to create db connection pool")
}

pub trait GraphqlApp {
    type Connection: 'static + diesel::Connection;
    type Adapter: Adapter<Self::Connection, Self::Query, Self::Mutation, Self::Context>;
    type Query;
    type Mutation;
    type Context;

    fn configure_web_framework(
        &self,
        web_framework: <Self::Adapter as Adapter<
            Self::Connection,
            Self::Query,
            Self::Mutation,
            Self::Context,
        >>::Inner,
    ) -> <Self::Adapter as Adapter<
        Self::Connection,
        Self::Query,
        Self::Mutation,
        Self::Context,
    >>::Inner {
        web_framework
    }

    fn graphql_path(&self) -> &'static str {
        "/graphql"
    }

    fn mount_graphql_at(&self) -> &'static str {
        "/"
    }

    fn mount_graphiql_at(&self) -> &'static str {
        "/"
    }

    fn database_connection_pool_max_size(&self) -> u32 {
        10
    }

    fn database_url_env_var(&self) -> &'static str {
        "DATABASE_URL"
    }

    fn database_url(&self) -> String {
        let var = self.database_url_env_var();
        std::env::var(var).expect(&format!("{} must be set", var))
    }
}

pub trait Adapter<Connection, Query, Mutation, Context>
where
    Self: Sized,
    Connection: 'static + diesel::Connection,
{
    type Inner;

    fn new() -> Self;

    fn run<App: GraphqlApp>(&self, app: App, config: WebFrameworkConfig<Connection>)
    where
        App: GraphqlApp<
            Adapter = Self,
            Connection = Connection,
            Query = Query,
            Mutation = Mutation,
            Context = Context,
        >;
}

pub struct WebFrameworkConfig<Connection: 'static + diesel::Connection> {
    database_connection_pool: Pool<ConnectionManager<Connection>>,
    graphql_path: &'static str,
    mount_graphiql_at: &'static str,
    mount_graphql_at: &'static str,
}
