#![feature(proc_macro_hygiene, decl_macro)]

extern crate rocket;

use diesel::prelude::*;
use gimme_graphql::{
    rocket_adapter::{
        rocket::{
            fairing::AdHoc,
            http::Status,
            request::{FromRequest, Outcome, Request},
            Rocket, State,
        },
        RocketAdapter,
    },
    run_graphql_app, ConnectionManager, GraphqlApp, Pool, PooledConnection,
};
use juniper_from_schema::graphql_schema;

graphql_schema! {
    schema {
        query: Query
        mutation: Mutation
    }

    type Query {
        helloWorld: String! @juniper(ownership: "owned")
    }

    type Mutation {
        noop: Boolean!
    }
}

pub struct Context {
    pub db_con: PooledConnection<ConnectionManager<PgConnection>>,
}

impl juniper::Context for Context {}

impl<'a, 'r> FromRequest<'a, 'r> for Context {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Context, ()> {
        let db_pool = request.guard::<State<Pool<ConnectionManager<PgConnection>>>>()?;

        match db_pool.get() {
            Ok(db_con) => Outcome::Success(Context { db_con }),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ())),
        }
    }
}

#[derive(Default)]
pub struct Query;

impl QueryFields for Query {
    fn field_hello_world(
        &self,
        _: &juniper::Executor<'_, Context>,
    ) -> juniper::FieldResult<String> {
        Ok(format!("Hello, World!"))
    }
}

#[derive(Default)]
pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, _: &juniper::Executor<'_, Context>) -> juniper::FieldResult<&bool> {
        Ok(&true)
    }
}

struct App;

impl GraphqlApp for App {
    type Adapter = RocketAdapter;
    type Connection = PgConnection;
    type Query = Query;
    type Mutation = Mutation;
    type Context = Context;

    fn configure_web_framework(&self, rocket: Rocket) -> Rocket {
        rocket.attach(AdHoc::on_request("Request Printer", |_req, _data| {
            log::debug!("Received request!");
        }))
    }
}

pub fn main() {
    run_graphql_app(App);
}
