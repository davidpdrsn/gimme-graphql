#[macro_use]
extern crate diesel;

use diesel::prelude::*;
use gimme_graphql::{
    hyper_adapter::{self, hyper, HyperAdapter},
    run_graphql_app, ConnectionManager, GraphqlApp, Pool,
};
use juniper::ID;
use juniper_from_schema::graphql_schema_from_file;

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

        let db = &executor.context().db_pool.get()?;

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
    type Adapter = HyperAdapter;
    type Connection = PgConnection;
    type Query = Query;
    type Mutation = Mutation;
    type Context = Context;
}

pub struct Context {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
}

impl juniper::Context for Context {}

impl hyper_adapter::CreateContext<PgConnection> for Context {
    fn create(
        db_pool: &Pool<ConnectionManager<PgConnection>>,
        _: &hyper::Request<hyper::Body>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Context {
            db_pool: db_pool.clone(),
        })
    }
}
