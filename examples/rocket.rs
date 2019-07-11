#![feature(proc_macro_hygiene, decl_macro)]

extern crate rocket;
#[macro_use]
extern crate diesel;

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
use juniper::ID;
use juniper_from_schema::graphql_schema_from_file;
use std::sync::Mutex;

mod schema {
    table! {
        users {
            id -> Integer,
        }
    }
}

graphql_schema_from_file!("examples/schema.graphql");

#[derive(Default)]
pub struct Query;

impl QueryFields for Query {
    fn field_users(
        &self,
        executor: &juniper::Executor<'_, Context>,
        _: &QueryTrail<'_, User, Walked>,
        limit: i32,
        offset: i32,
    ) -> juniper::FieldResult<Vec<User>> {
        use schema::users;

        let db = &executor.context().db_con;

        let users = users::table
            .limit(limit.into())
            .offset(offset.into())
            .load::<User>(db)?;

        Ok(users)
    }
}

#[derive(Queryable)]
pub struct User {
    id: i32,
}

impl UserFields for User {
    fn field_id(&self, _: &juniper::Executor<'_, Context>) -> juniper::FieldResult<ID> {
        Ok(ID::from(self.id.to_string()))
    }
}

#[derive(Default)]
pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, _: &juniper::Executor<'_, Context>) -> juniper::FieldResult<&bool> {
        Ok(&true)
    }
}

pub fn main() {
    run_graphql_app(App);
}

struct App;

impl GraphqlApp for App {
    type Adapter = RocketAdapter;
    type Connection = PgConnection;
    type Query = Query;
    type Mutation = Mutation;
    type Context = Context;
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
